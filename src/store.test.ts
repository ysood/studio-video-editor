import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Snapshot } from "./bindings/Snapshot";
import { sampleSnapshot } from "./preview/sampleProject";
import { useEditor } from "./store";

/** The sample project with a second, empty sequence added. */
function twoSequences(): Snapshot {
  const snapshot = sampleSnapshot();
  const first = snapshot.project.sequences[0]!;
  const second = { ...first, id: "sequence-2", name: "Sequence 2", tracks: [] };
  return { ...snapshot, project: { ...snapshot.project, sequences: [first, second] } };
}

describe("switching sequences", () => {
  beforeEach(() => {
    // These tests run under node, where there is no localStorage. The store treats it
    // as a convenience and works without it, but remembering is what we are checking.
    const saved = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => saved.get(key) ?? null,
      setItem: (key: string, value: string) => void saved.set(key, value),
      removeItem: (key: string) => void saved.delete(key),
      clear: () => saved.clear(),
    });
    useEditor.setState({ activeSequenceId: null, views: {}, playhead: 0, zoom: 4, scrollX: 0 });
  });

  it("keeps each sequence's playhead and zoom", () => {
    const snapshot = twoSequences();
    const store = useEditor.getState();
    store.setSnapshot(snapshot);

    // Move about in the first, then switch away and back.
    useEditor.getState().setPlayhead(120);
    useEditor.getState().setZoom(12);
    useEditor.getState().setActiveSequence("sequence-2");

    expect(useEditor.getState().playhead).toBe(0);
    expect(useEditor.getState().zoom).toBe(4);

    useEditor.getState().setActiveSequence(snapshot.project.sequences[0]!.id);
    expect(useEditor.getState().playhead).toBe(120);
    expect(useEditor.getState().zoom).toBe(12);
  });

  it("stays put when asked for the sequence already open", () => {
    const snapshot = twoSequences();
    useEditor.getState().setSnapshot(snapshot);
    useEditor.getState().setPlayhead(42);
    // Null means the first sequence, so naming it is not a switch.
    useEditor.getState().setActiveSequence(snapshot.project.sequences[0]!.id);
    expect(useEditor.getState().playhead).toBe(42);
  });

  it("reopens the project on the sequence it was left on", () => {
    const snapshot = twoSequences();
    useEditor.getState().setSnapshot(snapshot);
    useEditor.getState().setActiveSequence("sequence-2");

    // A fresh session: the store forgets, local storage does not.
    useEditor.setState({ activeSequenceId: null, views: {} });
    useEditor.getState().setSnapshot(snapshot);
    expect(useEditor.getState().activeSequenceId).toBe("sequence-2");
  });

  it("falls back when the remembered sequence has been deleted", () => {
    const snapshot = twoSequences();
    useEditor.getState().setSnapshot(snapshot);
    useEditor.getState().setActiveSequence("sequence-2");

    const without = {
      ...snapshot,
      project: { ...snapshot.project, sequences: [snapshot.project.sequences[0]!] },
    };
    useEditor.setState({ activeSequenceId: null, views: {} });
    useEditor.getState().setSnapshot(without);
    expect(useEditor.getState().activeSequenceId).toBeNull();
  });
});
