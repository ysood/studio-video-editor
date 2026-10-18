// What the app opens on, and what it returns to when a project is closed.
//
// Both reference editors do this: Premiere has a home screen listing recent projects,
// Resolve a Project Manager. The reason is the same in each: a project lives in a file
// somewhere, and once its window is closed the only way back is to remember where it
// went. The list here is that memory.

import { useCallback, useEffect, useState } from "react";

import { openProject } from "../actions";

import type { Recent } from "../bindings/Recent";
import { ipc } from "../ipc";
import { isBrowserPreview } from "../preview/mode";
import { useEditor } from "../store";
import { Button, Icon } from "../ui";
import "./home.css";

/** The folder a path sits in, which is what tells two projects of the same name apart. */
function folderOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut > 0 ? path.slice(0, cut) : path;
}

export function HomeScreen({ onNew }: { onNew: () => void }) {
  const [recents, setRecents] = useState<Recent[]>([]);
  const fail = useEditor((s) => s.fail);
  const setSnapshot = useEditor((s) => s.setSnapshot);

  const load = useCallback(() => {
    if (isBrowserPreview) return;
    void ipc
      .recentsList()
      .then(setRecents)
      .catch(() => setRecents([]));
  }, []);

  useEffect(load, [load]);

  const openPath = async (path: string) => {
    try {
      setSnapshot(await ipc.openProject(path));
    } catch (error) {
      fail(error);
      load();
    }
  };

  const forget = async (path: string) => {
    try {
      await ipc.recentsForget(path);
      load();
    } catch (error) {
      fail(error);
    }
  };

  return (
    <div className="home" data-surface="chrome">
      <div className="home-centre">
        <h1 className="home-wordmark">Studio</h1>

        <div className="home-actions">
          <Button variant="solid" onClick={onNew}>
            New project
          </Button>
          <Button variant="quiet" onClick={() => void openProject()}>
            Open project…
          </Button>
        </div>

        {recents.length > 0 && (
          <section className="home-recents" aria-label="Recent projects">
            <ul className="home-list">
              {recents.map((recent) => (
                <li key={recent.path} className="home-row" data-missing={!recent.exists}>
                  <button
                    type="button"
                    className="home-open"
                    disabled={!recent.exists}
                    onClick={() => void openPath(recent.path)}
                    title={recent.exists ? recent.path : `${recent.path} (not found)`}
                  >
                    <span className="home-name">{recent.name}</span>
                    <span className="home-where">
                      <bdi>{recent.exists ? folderOf(recent.path) : "Moved or deleted"}</bdi>
                    </span>
                  </button>
                  <button
                    type="button"
                    className="home-forget"
                    aria-label={`Remove ${recent.name} from the list`}
                    onClick={() => void forget(recent.path)}
                  >
                    <Icon name="close" />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}
