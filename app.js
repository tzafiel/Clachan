const GAELIC_PHASE = {
  dawn: "Madainn (Dawn Fog)",
  day: "Latha (Highland Day)",
  dusk: "Feasgar (Peat Dusk)",
  night: "Oidhche (Lantern Night)",
};

let skyOverride = "auto";
let dispatches = [];

function getSkyFromHour(hour) {
  if (hour >= 5 && hour < 9) return "dawn";
  if (hour >= 9 && hour < 17) return "day";
  if (hour >= 17 && hour < 21) return "dusk";
  return "night";
}

function updateClockAndSky() {
  const now = new Date();
  const phase = skyOverride === "auto" ? getSkyFromHour(now.getHours()) : skyOverride;
  document.body.dataset.sky = phase;

  const clockEl = document.getElementById("localClock");
  const gaelicEl = document.getElementById("gaelicTime");
  if (clockEl) {
    clockEl.textContent = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  if (gaelicEl) {
    gaelicEl.textContent = GAELIC_PHASE[phase];
  }
}

function spawnEmbers() {
  const layer = document.getElementById("embersLayer");
  if (!layer) return;
  for (let i = 0; i < 18; i++) {
    const ember = document.createElement("span");
    ember.className = "ember";
    ember.style.left = `${Math.random() * 100}%`;
    ember.style.animationDuration = `${7 + Math.random() * 9}s`;
    ember.style.animationDelay = `${Math.random() * 8}s`;
    layer.appendChild(ember);
  }
}

function openDispatchModal(entry) {
  if (!entry) return;
  document.getElementById("modalWeather").textContent = `Weather: ${entry.weather}`;
  document.getElementById("modalDate").textContent = `${entry.label} · ${entry.date}`;
  document.getElementById("modalGaelic").textContent = entry.gaelic_opener;
  document.getElementById("modalTitle").textContent = entry.title;
  document.getElementById("modalSignoff").textContent = entry.signoff;

  const bodyEl = document.getElementById("modalBody");
  bodyEl.innerHTML = "";
  entry.paragraphs.forEach((text) => {
    const p = document.createElement("p");
    p.textContent = text;
    bodyEl.appendChild(p);
  });

  document.getElementById("dispatchModal").hidden = false;
}

function closeDispatchModal() {
  document.getElementById("dispatchModal").hidden = true;
}

async function loadDispatches() {
  const grid = document.getElementById("dispatchGrid");
  try {
    const res = await fetch("dispatches/entries.json");
    dispatches = await res.json();
    grid.innerHTML = "";

    dispatches.forEach((entry) => {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "dispatch-card";
      card.innerHTML = `
        <div class="dispatch-meta">
          <span>${entry.label}</span>
          <span>${entry.weather}</span>
        </div>
        <h3>${entry.title}</h3>
        <p class="dispatch-gaelic">${entry.gaelic_opener}</p>
        <p>${entry.excerpt}</p>
        <span class="dispatch-read">Unroll Scroll →</span>
      `;
      card.addEventListener("click", () => openDispatchModal(entry));
      grid.appendChild(card);
    });
  } catch (err) {
    grid.innerHTML = `<p style="color:var(--text-muted)">The Watch-Falcon is still crossing the mist. (Run via a local server or GitHub Pages to load <code>dispatches/entries.json</code>.)</p>`;
  }
}

document.addEventListener("DOMContentLoaded", () => {
  updateClockAndSky();
  setInterval(updateClockAndSky, 30000);
  spawnEmbers();
  loadDispatches();

  document.querySelectorAll(".sky-preview button").forEach((btn) => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".sky-preview button").forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      skyOverride = btn.dataset.preview;
      updateClockAndSky();
    });
  });

  document.getElementById("openLatestDispatch")?.addEventListener("click", () => {
    if (dispatches.length > 0) openDispatchModal(dispatches[0]);
  });

  document.getElementById("closeModal")?.addEventListener("click", closeDispatchModal);
  document.getElementById("dispatchModal")?.addEventListener("click", (e) => {
    if (e.target.id === "dispatchModal") closeDispatchModal();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeDispatchModal();
  });
});
