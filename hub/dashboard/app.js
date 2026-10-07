// Home Hub dashboard v0 (M1): Overview / Devices / Storage + Pair QR.
const H = { "X-HH-Local": window.HH_TOKEN };

function fmtBytes(n) {
  if (n == null) return "–";
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(n >= 100 || i === 0 ? 0 : 1)} ${u[i]}`;
}

async function api(path, opts = {}) {
  const r = await fetch(path, { ...opts, headers: { ...H, ...(opts.headers || {}) } });
  if (!r.ok) throw new Error(`${path}: ${r.status}`);
  return r;
}

// --- nav ---
document.querySelectorAll(".nav-link").forEach(a => {
  a.addEventListener("click", e => {
    e.preventDefault();
    const viewName = a.dataset.view;
    document.querySelectorAll(".nav-link").forEach(x => {
      if (x.dataset.view === viewName && x.parentElement.tagName === "NAV") {
        x.classList.add("active");
      } else if (x.parentElement.tagName === "NAV") {
        x.classList.remove("active");
      }
    });
    document.querySelectorAll(".view").forEach(v => (v.hidden = true));
    const target = document.getElementById(`view-${viewName}`);
    if (target) target.hidden = false;
    if (viewName === "files") loadFiles();
    if (viewName === "overview") { loadOverview(); loadRecentFiles(); }
  });
});

// --- overview ---
async function loadOverview() {
  const o = await (await api("/api/overview")).json();
  document.getElementById("hub-name").textContent = o.name;
  document.getElementById("hub-version").textContent = o.version;
  document.getElementById("free-space").textContent = `${fmtBytes(o.free_bytes)} free`;
  const pct = o.total_bytes ? (1 - o.free_bytes / o.total_bytes) * 100 : 0;
  document.getElementById("storage-bar").style.width = `${pct}%`;
  document.getElementById("copies").textContent =
    o.copies >= 2 ? "2 copies ✓" : "1 copy only — add an external drive for a safe second copy";
  document.getElementById("device-count").textContent = o.devices;
  document.getElementById("active-transfers").textContent = o.active_transfers;

  const alerts = await (await api("/api/alerts")).json();
  const card = document.getElementById("alerts-card");
  const list = document.getElementById("alerts-list");
  list.innerHTML = "";
  card.hidden = alerts.items.length === 0;
  for (const a of alerts.items) {
    const li = document.createElement("li");
    li.innerHTML = `<span class="chip ${a.severity === "critical" ? "danger" : "warn"}">${a.severity}</span> ${a.message}`;
    list.appendChild(li);
  }
}

// --- pair QR ---
document.getElementById("btn-pair").addEventListener("click", async () => {
  await api("/api/pair/open", { method: "POST" });
  await showQr();
});

async function showQr() {
  const r = await api("/api/pair/qr");
  const svgB64 = r.headers.get("x-qr-svg");
  const meta = await r.json();
  const area = document.getElementById("qr-area");
  const svg = atob(svgB64);
  area.innerHTML = svg;
  const metaEl = document.getElementById("pair-meta");
  metaEl.textContent = `Code expires in ${meta.expires_in}s · Or use manual code: ${meta.manual_code}`;
}

// --- devices ---
async function loadDevices() {
  const d = await (await api("/api/devices")).json();
  const body = document.getElementById("devices-body");
  body.innerHTML = "";
  for (const dev of d.items) {
    const tr = document.createElement("tr");
    const seen = dev.last_seen_at ? new Date(dev.last_seen_at).toLocaleString() : "never";
    tr.innerHTML = `<td>${dev.name}</td><td>${dev.platform}</td><td>${dev.scopes.join(", ")}</td>
      <td><span class="chip ${dev.status === "active" ? "ok" : "danger"}">${dev.status}</span></td><td>${seen}</td>`;
    body.appendChild(tr);
  }
}

// --- storage ---
async function loadStorage() {
  const s = await (await api("/api/storage")).json();
  const cats = document.getElementById("cat-list");
  cats.innerHTML = "";
  for (const c of s.categories) {
    const li = document.createElement("li");
    li.textContent = `${c.category}: ${c.count} items · ${fmtBytes(c.bytes)}`;
    cats.appendChild(li);
  }
  const disks = document.getElementById("disk-list");
  disks.innerHTML = "";
  for (const d of s.disks) {
    const li = document.createElement("li");
    const chip = d.health === "good" ? "ok" : d.health === "caution" ? "warn" : d.health === "failing" ? "danger" : "";
    li.innerHTML = `<span class="chip ${chip}">${d.health}</span> ${d.model || d.disk_id}` +
      (d.temperature_c != null ? ` · ${d.temperature_c}°C` : "");
    disks.appendChild(li);
  }
}

// --- files & photos ---
function createFileCard(f) {
  const card = document.createElement("div");
  card.className = "file-card";
  const isPhoto = f.category === "photo" || (f.mime && f.mime.startsWith("image/"));
  const previewHtml = isPhoto
    ? `<img src="/api/files/${f.id}/thumb" class="file-thumb" alt="${f.name}" loading="lazy" onerror="this.src='/api/files/${f.id}/content'">`
    : `<div class="file-icon">📄</div>`;
  const date = f.created_at ? new Date(f.created_at).toLocaleDateString() : "";
  card.innerHTML = `
    <div class="file-preview">
      ${previewHtml}
    </div>
    <div class="file-info">
      <div class="file-name" title="${f.name}">${f.name}</div>
      <div class="file-meta">
        <span>${fmtBytes(f.size)}</span>
        <span>${date}</span>
      </div>
      <a href="/api/files/${f.id}/content" target="_blank" download="${f.name}" class="btn-download">
        Download / View
      </a>
    </div>
  `;
  return card;
}

async function loadFiles() {
  try {
    const res = await (await api("/api/files")).json();
    const grid = document.getElementById("file-grid");
    const empty = document.getElementById("files-empty");
    if (!grid) return;
    grid.innerHTML = "";
    if (!res.items || res.items.length === 0) {
      if (empty) empty.hidden = false;
      return;
    }
    if (empty) empty.hidden = true;
    for (const f of res.items) {
      grid.appendChild(createFileCard(f));
    }
  } catch (e) {
    console.error("loadFiles error", e);
  }
}

async function loadRecentFiles() {
  try {
    const res = await (await api("/api/files")).json();
    const grid = document.getElementById("overview-recent-files");
    const empty = document.getElementById("overview-no-files");
    if (!grid) return;
    grid.innerHTML = "";
    if (!res.items || res.items.length === 0) {
      if (empty) empty.hidden = false;
      return;
    }
    if (empty) empty.hidden = true;
    const recents = res.items.slice(0, 4);
    for (const f of recents) {
      grid.appendChild(createFileCard(f));
    }
  } catch (e) {
    console.error("loadRecentFiles error", e);
  }
}

document.getElementById("btn-refresh-files")?.addEventListener("click", loadFiles);

loadOverview();
loadRecentFiles();
loadDevices();
loadStorage();
loadFiles();
setInterval(loadOverview, 5000);
setInterval(loadRecentFiles, 5000);

