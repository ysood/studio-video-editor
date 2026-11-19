// What the app opens on, and what it returns to when a project is closed.
//
// Both reference editors do this: Premiere has a home screen listing recent projects,
// Resolve a Project Manager. The reason is the same in each: a project lives in a file
// somewhere, and once its window is closed the only way back is to remember where it
// went. The list here is that memory.

import { useCallback, useEffect, useState } from "react";

import { openProject } from "../actions";
import type { Recent } from "../bindings/Recent";
import { timeAgo } from "../format";
import { ipc } from "../ipc";
import { useEditor } from "../store";
import { Button, Icon, IconButton, List, PlainButton, Row } from "../ui";
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
        <header className="home-mark">
          <h1 className="home-wordmark">Studio</h1>
          <p className="home-tagline">Video editing studio</p>
        </header>

        <div className="home-actions">
          <Button variant="solid" size="large" onClick={onNew}>
            New project
            <Icon name="arrowRight" />
          </Button>
          <Button variant="quiet" size="large" onClick={() => void openProject()}>
            Open project…
          </Button>
        </div>

        {recents.length > 0 && (
          <section className="home-recents" aria-label="Recent projects">
            <h2 className="home-section">
              <span>Recent projects</span>
            </h2>
            <List className="home-list">
              {recents.map((recent) => (
                <Row key={recent.path} marker="none" className="home-row" data-missing={!recent.exists}>
                  <PlainButton
                    className="home-open"
                    disabled={!recent.exists}
                    onClick={() => void openPath(recent.path)}
                    title={recent.exists ? recent.path : `${recent.path} (not found)`}
                  >
                    <span className="home-name">{recent.name}</span>
                    <span className="home-where">
                      <bdi>{recent.exists ? folderOf(recent.path) : "Moved or deleted"}</bdi>
                    </span>
                    <span className="home-when">{timeAgo(Number(recent.openedUnixMs))}</span>
                  </PlainButton>
                  <IconButton
                    icon="close"
                    size="small"
                    className="home-forget"
                    label={`Remove ${recent.name} from the list`}
                    onClick={() => void forget(recent.path)}
                  />
                </Row>
              ))}
            </List>
          </section>
        )}
      </div>
    </div>
  );
}
