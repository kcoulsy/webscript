(() => {
  const TASK_CLASSES = {
    model: "ws-debugbar-task-model",
    db: "ws-debugbar-task-db",
    sleep: "ws-debugbar-task-sleep",
    fetch: "ws-debugbar-task-fetch",
    timeout: "ws-debugbar-task-timeout",
    spawn: "ws-debugbar-task-spawn",
    await: "ws-debugbar-task-await",
  };

  const escapeHtml = (value) =>
    String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");

  const truncate = (value, max) => {
    const text = String(value);
    if (text.length <= max) return text;
    return `${text.slice(0, max - 1)}…`;
  };

  const timelineMs = (metrics) => {
    const taskEnd = (metrics.tasks || []).reduce(
      (max, task) => Math.max(max, (task.startMs || 0) + (task.durationMs || 0)),
      0,
    );
    return Math.max(taskEnd, metrics.total || 0, 1);
  };

  const ganttTicks = (maxMs) => {
    const step =
      maxMs <= 20 ? 5 : maxMs <= 100 ? 10 : maxMs <= 500 ? 50 : maxMs <= 2000 ? 200 : 500;
    const ticks = [];
    for (let value = 0; value <= maxMs; value += step) {
      ticks.push(value);
    }
    if (ticks[ticks.length - 1] !== maxMs) {
      ticks.push(maxMs);
    }
    return ticks;
  };

  const formatTime = (timestamp) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  const renderSummaryPills = (metrics) => {
    const pills = (metrics.entries || []).map(
      (entry) =>
        `<span class="ws-debugbar-pill">${escapeHtml(entry.label)}: ${entry.duration} ms</span>`,
    );
    if ((metrics.tasks || []).length > 0) {
      pills.push(`<span class="ws-debugbar-pill">Tasks: ${metrics.tasks.length}</span>`);
    }
    if ((metrics.queries || []).length > 0) {
      pills.push(`<span class="ws-debugbar-pill">Queries: ${metrics.queries.length}</span>`);
    }
    return pills.join("");
  };

  const renderTimings = (metrics) => {
    const rows = (metrics.entries || [])
      .map(
        (entry) => `<tr>
      <td>${escapeHtml(entry.label)}</td>
      <td>${entry.duration} ms</td>
      <td class="ws-debugbar-detail">${escapeHtml(entry.detail || "")}</td>
    </tr>`,
      )
      .join("");
    const routeRow =
      metrics.routeFile && metrics.routeFile !== ""
        ? `<tr><td>Route file</td><td colspan="2">${escapeHtml(metrics.routeFile)}</td></tr>`
        : "";
    return `<table>
      <thead>
        <tr><th>Phase</th><th>Time</th><th>Detail</th></tr>
      </thead>
      <tbody>
        <tr><td>Request</td><td colspan="2">${escapeHtml(metrics.requestPath || "")}</td></tr>
        ${routeRow}
        ${rows}
        <tr><td>Total</td><td colspan="2">${metrics.total || 0} ms</td></tr>
      </tbody>
    </table>`;
  };

  const renderTimeline = (metrics) => {
    const tasks = metrics.tasks || [];
    if (tasks.length === 0) {
      return '<p class="ws-debugbar-empty">No async tasks recorded.</p>';
    }
    const maxMs = timelineMs(metrics);
    const ticks = ganttTicks(maxMs)
      .map((ms) => {
        const left = (ms / maxMs) * 100;
        return `<span class="ws-debugbar-gantt-tick" style="left:${left.toFixed(2)}%">${ms} ms</span>`;
      })
      .join("");
    const rows = tasks
      .map((task) => {
        const left = ((task.startMs || 0) / maxMs) * 100;
        const width = Math.max(((task.durationMs || 0) / maxMs) * 100, 0.4);
        const cssClass = TASK_CLASSES[task.kind] || TASK_CLASSES.await;
        const title = escapeHtml(`${task.label} — ${task.durationMs} ms`);
        const label = escapeHtml(truncate(task.label, 80));
        return `<div class="ws-debugbar-gantt-row">
  <div class="ws-debugbar-gantt-label" title="${title}">${label}</div>
  <div class="ws-debugbar-gantt-track">
    <div class="ws-debugbar-gantt-bar ${cssClass}" style="left:${left.toFixed(2)}%;width:${width.toFixed(2)}%" title="${title}"></div>
  </div>
</div>`;
      })
      .join("");
    return `<div class="ws-debugbar-gantt">
  <div class="ws-debugbar-gantt-axis">
    <div></div>
    <div class="ws-debugbar-gantt-axis-track">${ticks}</div>
  </div>
  ${rows}
</div>`;
  };

  const renderQueries = (metrics) => {
    const queries = metrics.queries || [];
    if (queries.length === 0) {
      return '<p class="ws-debugbar-empty">No database queries recorded.</p>';
    }
    const rows = queries
      .map((query) => {
        const errorRow =
          query.error && query.error !== ""
            ? `<tr><td></td><td colspan="4" class="ws-debugbar-error">${escapeHtml(query.error)}</td></tr>`
            : "";
        return `<tr>
      <td>${query.duration} ms</td>
      <td>${escapeHtml(query.source || "")}</td>
      <td class="ws-debugbar-sql">${escapeHtml(query.sql || "")}</td>
      <td class="ws-debugbar-detail">${escapeHtml(query.params || "")}</td>
      <td>${escapeHtml(query.status || "")}</td>
    </tr>${errorRow}`;
      })
      .join("");
    return `<table>
      <thead>
        <tr><th>Time</th><th>Source</th><th>SQL</th><th>Params</th><th>Status</th></tr>
      </thead>
      <tbody>${rows}</tbody>
    </table>`;
  };

  const renderSession = (state) => {
    if (state.history.length === 0) {
      return '<p class="ws-debugbar-empty">No requests recorded this session.</p>';
    }
    const rows = [...state.history]
      .reverse()
      .map((entry) => {
        const selected = entry.id === state.activeId ? ' aria-current="true"' : "";
        const clientMs =
          entry.clientDurationMs != null
            ? `<td>${Math.round(entry.clientDurationMs)} ms</td>`
            : "<td>—</td>";
        return `<tr class="ws-debugbar-session-row" data-ws-debugbar-request="${entry.id}"${selected}>
      <td>${escapeHtml(entry.kind)}</td>
      <td>${escapeHtml(entry.path)}</td>
      <td>${entry.metrics.total || 0} ms</td>
      ${clientMs}
      <td>${(entry.metrics.queries || []).length}</td>
      <td>${(entry.metrics.tasks || []).length}</td>
      <td>${formatTime(entry.timestamp)}</td>
    </tr>`;
      })
      .join("");
    return `<table class="ws-debugbar-session-table">
      <thead>
        <tr><th>Type</th><th>Path</th><th>Server</th><th>Client</th><th>Queries</th><th>Tasks</th><th>Time</th></tr>
      </thead>
      <tbody>${rows}</tbody>
    </table>`;
  };

  const renderTabs = (activeTab) => {
    const tab = (name, label) => {
      const selected = activeTab === name;
      return `<button type="button" role="tab" aria-selected="${selected ? "true" : "false"}" data-ws-debugbar-tab="${name}">${label}</button>`;
    };
    return `<div class="ws-debugbar-tabs" role="tablist" aria-label="WebScript devtools">
      ${tab("timings", "Timings")}
      ${tab("timeline", "Async Timeline")}
      ${tab("queries", "Queries")}
      ${tab("session", "Session")}
    </div>`;
  };

  const renderPanel = (state) => {
    const entry = state.history.find((item) => item.id === state.activeId);
    if (!entry) {
      return '<p class="ws-debugbar-empty">No request selected.</p>';
    }
    let body = "";
    if (state.activeTab === "timings") {
      body = renderTimings(entry.metrics);
    } else if (state.activeTab === "timeline") {
      body = renderTimeline(entry.metrics);
    } else if (state.activeTab === "queries") {
      body = renderQueries(entry.metrics);
    } else {
      body = renderSession(state);
    }
    return `${renderTabs(state.activeTab)}<section class="ws-debugbar-tabpanel" role="tabpanel">${body}</section>`;
  };

  const parseMetricsNode = (root) => {
    const node = root.querySelector("#ws-request-metrics");
    if (!node) return null;
    try {
      return JSON.parse(node.textContent || "");
    } catch {
      return null;
    }
  };

  const stripDebugbar = (root) => {
    root.getElementById?.("webscript-debugbar")?.remove();
    root.getElementById?.("ws-request-metrics")?.remove();
    for (const node of root.querySelectorAll?.("#webscript-debugbar, #ws-request-metrics") || []) {
      node.remove();
    }
  };

  window.WebScript = window.WebScript || {};

  const state = {
    history: [],
    activeId: null,
    activeTab: "timings",
    nextId: 1,
  };

  const getBar = () => document.getElementById("webscript-debugbar");

  const getActiveEntry = () => state.history.find((item) => item.id === state.activeId) || null;

  const updateView = () => {
    const bar = getBar();
    if (!bar) return;
    const entry = getActiveEntry();
    const summary = bar.querySelector("[data-ws-debugbar-summary]");
    const panel = bar.querySelector(".ws-debugbar-panel");
    if (!summary || !panel) return;

    const total = entry?.metrics.total || 0;
    const path = entry?.path || "";
    const pills = entry ? renderSummaryPills(entry.metrics) : "";
    const sessionPill =
      state.history.length > 0
        ? `<span class="ws-debugbar-pill ws-debugbar-session-pill" data-ws-debugbar-session-pill>${state.history.length} request${state.history.length === 1 ? "" : "s"}</span>`
        : "";
    summary.innerHTML = `<span class="ws-debugbar-brand">WebScript</span>
      <span class="ws-debugbar-pill ws-debugbar-total">Total: ${total} ms</span>
      <span class="ws-debugbar-pill ws-debugbar-path" title="${escapeHtml(path)}">${escapeHtml(truncate(path || "/", 48))}</span>
      ${pills}
      ${sessionPill}
      <span class="ws-debugbar-toggle">&#9650;</span>`;
    panel.innerHTML = renderPanel(state);
  };

  const record = (metrics, options = {}) => {
    if (!metrics || typeof metrics !== "object") return null;
    const id = state.nextId++;
    const entry = {
      id,
      kind: options.kind || "navigation",
      path: options.path || metrics.requestPath || "",
      timestamp: options.timestamp || Date.now(),
      clientDurationMs: options.clientDurationMs ?? null,
      metrics,
    };
    state.history.push(entry);
    state.activeId = id;
    if (options.tab) {
      state.activeTab = options.tab;
    }
    updateView();
    return entry;
  };

  const select = (id, tab) => {
    if (!state.history.some((entry) => entry.id === id)) return;
    state.activeId = id;
    if (tab) {
      state.activeTab = tab;
    }
    updateView();
  };

  const initFromDocument = (doc = document) => {
    const metrics = parseMetricsNode(doc);
    if (metrics) {
      record(metrics, { kind: "document", path: metrics.requestPath || window.location.pathname });
    }
  };

  WebScript.devtools = {
    state,
    record,
    select,
    parseMetricsNode,
    stripDebugbar,
    initFromDocument,
    updateView,
  };

  document.addEventListener("click", (event) => {
    const bar = getBar();
    if (!bar) return;

    const tabButton = event.target.closest("[data-ws-debugbar-tab]");
    if (tabButton && bar.contains(tabButton)) {
      event.stopPropagation();
      state.activeTab = tabButton.getAttribute("data-ws-debugbar-tab") || "timings";
      updateView();
      return;
    }

    const sessionRow = event.target.closest("[data-ws-debugbar-request]");
    if (sessionRow && bar.contains(sessionRow)) {
      event.stopPropagation();
      const id = Number(sessionRow.getAttribute("data-ws-debugbar-request"));
      select(id, "timings");
      return;
    }

    const sessionPill = event.target.closest("[data-ws-debugbar-session-pill]");
    if (sessionPill && bar.contains(sessionPill)) {
      event.stopPropagation();
      state.activeTab = "session";
      bar.classList.add("open");
      updateView();
      return;
    }

    const toggle = event.target.closest("[data-ws-debugbar-toggle]");
    if (toggle && bar.contains(toggle)) {
      bar.classList.toggle("open");
    }
  });

  initFromDocument();
})();
