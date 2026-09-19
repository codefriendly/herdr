// installed by herdr
// managed by herdr; reinstalling or updating the integration overwrites this file.
// add custom hooks/plugins beside this file instead of editing it.
// HERDR_INTEGRATION_ID=amp
// HERDR_INTEGRATION_VERSION=1
// @ts-nocheck

import type { PluginAPI } from "@ampcode/plugin";
import net from "node:net";

const SOURCE = "herdr:amp";
const AGENT = "amp";

function enabled(): boolean {
  return (
    process.env.HERDR_ENV === "1" &&
    !!process.env.HERDR_SOCKET_PATH &&
    !!process.env.HERDR_PANE_ID
  );
}

function reportSession(threadId: string): Promise<void> {
  const paneId = process.env.HERDR_PANE_ID;
  const socketPath = process.env.HERDR_SOCKET_PATH;
  if (!enabled() || !paneId || !socketPath || !threadId.startsWith("T-")) {
    return Promise.resolve();
  }

  const socketEndpoint =
    process.platform === "win32" ? `\\\\.\\pipe\\${socketPath}` : socketPath;
  const request = {
    id: `${SOURCE}:${Date.now()}:${Math.random().toString(36).slice(2)}`,
    method: "pane.report_agent_session",
    params: {
      pane_id: paneId,
      source: SOURCE,
      agent: AGENT,
      agent_session_id: threadId,
      session_start_source: "select",
    },
  };

  return new Promise((resolve) => {
    const socket = net.createConnection(socketEndpoint, () => {
      socket.write(`${JSON.stringify(request)}\n`);
    });
    const finish = () => {
      socket.destroy();
      resolve();
    };
    socket.setTimeout(500, finish);
    socket.on("data", finish);
    socket.on("error", finish);
    socket.on("end", finish);
    socket.on("close", resolve);
  });
}

export default function (amp: PluginAPI): void {
  if (!enabled()) {
    return;
  }

  amp.on("session.start", async (event) => {
    const activeThread = amp.activeThread.current;
    if (activeThread && activeThread.id !== event.thread.id) {
      return;
    }
    await reportSession(event.thread.id);
  });
}
