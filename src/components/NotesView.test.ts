import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { JSDOM } from "jsdom";
import { createServer, type ViteDevServer } from "vite";

/**
 * Mount tests for NotesView. jsdom provides the DOM; Vite's SSR loader handles
 * TSX and `import.meta.env`. React's `act` flushes the effects where the old
 * sync/async service mismatch used to throw.
 */
const dom = new JSDOM("<!doctype html><html><body></body></html>", {
  url: "http://localhost/",
});

const globals = globalThis as Record<string, unknown>;
globals.window = dom.window;
globals.document = dom.window.document;
Object.defineProperty(globalThis, "navigator", {
  value: dom.window.navigator,
  configurable: true,
});
globals.HTMLElement = dom.window.HTMLElement;
globals.Element = dom.window.Element;
globals.Node = dom.window.Node;
globals.localStorage = dom.window.localStorage;
globals.getComputedStyle = dom.window.getComputedStyle.bind(dom.window);
globals.requestAnimationFrame = (callback: FrameRequestCallback) =>
  setTimeout(() => callback(Date.now()), 0) as unknown as number;
globals.cancelAnimationFrame = (handle: number) => clearTimeout(handle);
globals.IS_REACT_ACT_ENVIRONMENT = true;
dom.window.scrollTo = () => undefined;

dom.window.matchMedia =
  dom.window.matchMedia ||
  ((query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => undefined,
      removeListener: () => undefined,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      dispatchEvent: () => false,
    }) as unknown as MediaQueryList);

type NotesViewModule = typeof import("./NotesView");
type ErrorBoundaryModule = typeof import("./ErrorBoundary");
type ServicesModule = typeof import("../services/index");
type SessionModule = typeof import("../notes/noteSession");
type ReactModule = typeof import("react");
type ClientModule = typeof import("react-dom/client");

let vite: ViteDevServer;
let NotesView: NotesViewModule["NotesView"];
let noteCardHeightForContent: NotesViewModule["noteCardHeightForContent"];
let ErrorBoundary: ErrorBoundaryModule["ErrorBoundary"];
let services: ServicesModule;
let createNoteSession: SessionModule["createNoteSession"];
let React: ReactModule;
let client: ClientModule;
let originalList: ServicesModule["noteService"]["list"];

before(async () => {
  vite = await createServer({
    configFile: false,
    appType: "custom",
    logLevel: "error",
    // Tests never need HMR, and watching the project (including the 35 GB
    // `src-tauri/target` tree) stalls this server until module fetches time
    // out. Disable the watcher outright.
    server: { middlewareMode: true, watch: null },
    optimizeDeps: { noDiscovery: true },
    // @ts-expect-error Vite's ESBuildOptions type omits `jsx`; the runtime honours it.
    esbuild: { jsx: "automatic" },
  });
  ({ NotesView, noteCardHeightForContent } = await vite.ssrLoadModule(
    "/src/components/NotesView.tsx",
  ) as NotesViewModule);
  ({ ErrorBoundary } = await vite.ssrLoadModule(
    "/src/components/ErrorBoundary.tsx",
  ) as ErrorBoundaryModule);
  services = await vite.ssrLoadModule("/src/services/index.ts") as ServicesModule;
  ({ createNoteSession } = await vite.ssrLoadModule(
    "/src/notes/noteSession.ts",
  ) as SessionModule);
  React = await import("react");
  client = await import("react-dom/client");
  originalList = services.noteService.list;
});

after(async () => {
  services.noteService.list = originalList;
  await vite?.close();
});

function makeSession() {
  return createNoteSession({
    getSettings: () => ({
      notesLastId: null,
      notesLastEditedMs: 0,
      notesNewAfterMinutes: 720,
    }),
    persist: () => undefined,
  });
}

async function mount(session = makeSession()) {
  const container = dom.window.document.createElement("div");
  dom.window.document.body.appendChild(container);
  const root = client.createRoot(container);
  await React.act(async () => {
    root.render(
      React.createElement(NotesView, {
        session,
        onHeightChange: () => undefined,
      }),
    );
  });
  return {
    container,
    session,
    unmount: async () => {
      await React.act(async () => root.unmount());
      container.remove();
    },
  };
}

function BrokenView(): never {
  throw new Error("view exploded");
}

test("NotesView mounts with no notes", async () => {
  services.noteService.list = () => [];
  const view = await mount();
  try {
    assert.equal(view.container.querySelector("textarea")?.value, "");
    assert.equal(view.container.querySelector("input")?.value, "");
    assert.ok(view.container.textContent?.includes("Saved"));
  } finally {
    await view.unmount();
  }
});

test("Notes editor grows only for new body lines and caps below the panel maximum", () => {
  assert.equal(noteCardHeightForContent(0), 120);
  assert.equal(noteCardHeightForContent(42), 120);
  assert.equal(noteCardHeightForContent(43), 120);
  assert.equal(noteCardHeightForContent(180), 248);
  // The remaining outer panel chrome takes the panel to its 340 px maximum.
  assert.equal(noteCardHeightForContent(10_000), 288);
});

test("Notes session preserves blank lines in the multiline body", () => {
  const session = makeSession();
  session.update({ body: "First paragraph\n\nSecond paragraph" });
  assert.equal(session.getSnapshot().body, "First paragraph\n\nSecond paragraph");
});

test("NotesView mounts when the note list fails to load", async () => {
  services.noteService.list = () => {
    throw new Error("notes list unavailable");
  };
  const thrown = await mount();
  try {
    assert.ok(thrown.container.querySelector("textarea"));
  } finally {
    await thrown.unmount();
  }

  services.noteService.list = () => Promise.reject(new Error("notes list unavailable"));
  const rejected = await mount();
  try {
    assert.ok(rejected.container.querySelector("textarea"));
    assert.ok(!rejected.container.textContent?.includes("NaN"));
  } finally {
    await rejected.unmount();
  }
});

test("NotesView mounts a note with missing fields", async () => {
  services.noteService.list = (() => [
    {},
    { id: "partial", title: null, body: undefined, modifiedMs: "yesterday" },
  ]) as unknown as ServicesModule["noteService"]["list"];
  const view = await mount();
  try {
    assert.equal(view.container.querySelector("input")?.value, "");
    assert.equal(view.container.querySelector("textarea")?.value, "");
    assert.ok(!view.container.textContent?.includes("NaN"));
  } finally {
    await view.unmount();
  }
});

test("NotesView survives corrupt mock note storage", async () => {
  services.noteService.list = originalList;
  dom.window.localStorage.setItem("notch-notes", JSON.stringify({ notes: "not an array" }));
  const view = await mount();
  try {
    assert.ok(view.container.querySelector("textarea"));
  } finally {
    await view.unmount();
    dom.window.localStorage.removeItem("notch-notes");
  }
});

test("NotesView renders from the session and keeps text across remounts", async () => {
  services.noteService.list = () => [];
  let savedId: string | null = null;
  const session = createNoteSession({
    getSettings: () => ({
      notesLastId: null,
      notesLastEditedMs: 0,
      notesNewAfterMinutes: 720,
    }),
    persist: (id) => {
      savedId = id;
    },
    service: {
      get: () => undefined,
      save: async (id, _title, _body) => {
        const nextId = id ?? "note-draft";
        savedId = nextId;
        return { id: nextId, modifiedMs: 123 };
      },
    },
    autosaveMs: 60_000,
  });

  const first = await mount(session);
  try {
    await React.act(async () => {
      session.update({ body: "Typed text survives a tab switch" });
    });
    assert.equal(
      first.container.querySelector("textarea")?.value,
      "Typed text survives a tab switch",
    );
    await React.act(async () => {
      await session.flush();
    });
    assert.equal(savedId, "note-draft");
  } finally {
    await first.unmount();
  }

  const second = await mount(session);
  try {
    assert.equal(
      second.container.querySelector("textarea")?.value,
      "Typed text survives a tab switch",
    );
  } finally {
    await second.unmount();
  }
});

test("ErrorBoundary shows a safe fallback and keeps the panel height valid", async () => {
  const heights: number[] = [];
  const container = dom.window.document.createElement("div");
  dom.window.document.body.appendChild(container);
  const root = client.createRoot(container);
  const originalError = console.error;
  console.error = () => undefined; // React logs caught render errors in dev.
  try {
    await React.act(async () => {
      root.render(
        React.createElement(
          ErrorBoundary,
          {
            tab: "Notes",
            showOpenNotesFolder: true,
            onHeightChange: (height: number) => heights.push(height),
          },
          React.createElement(BrokenView),
        ),
      );
    });
    assert.ok(container.textContent?.includes("Something went wrong in Notes"));
    assert.ok(container.textContent?.includes("Try again"));
    assert.ok(container.textContent?.includes("Open notes folder"));
    assert.deepEqual(heights, [172]);
  } finally {
    console.error = originalError;
    await React.act(async () => root.unmount());
    container.remove();
  }
});
