const API = window.MPULSE_API || "http://localhost:3001";

const { t, humanError, formatClientDateTime } = window.MPulseI18n;

let captchaId = null;
let captchaQuestionText = "…";
let lastActivationCode = null;
let lastActivationExpiresAt = null;
let publicPlans = [];
let betaConfig = { selfService: false, maxDevices: 10 };
let selectedPlanId = null;
let dashboardDeviceLimit = 1;

function openModal(id) {
  const modal = document.getElementById(id);
  if (!modal) return;
  modal.hidden = false;
  modal.setAttribute("aria-hidden", "false");
  document.body.classList.add("modal-open");
  const firstInput = modal.querySelector("input:not([type=hidden])");
  if (firstInput) firstInput.focus();
}

function closeModal(id) {
  const modal = document.getElementById(id);
  if (!modal) return;
  modal.hidden = true;
  modal.setAttribute("aria-hidden", "true");
  if (!document.querySelector(".modal:not([hidden])")) {
    document.body.classList.remove("modal-open");
  }
}

function closeAllModals() {
  document.querySelectorAll(".modal").forEach((modal) => {
    modal.hidden = true;
    modal.setAttribute("aria-hidden", "true");
  });
  document.body.classList.remove("modal-open");
}

function showModalMessage(modalId, text, isError = false) {
  const suffix = modalId.replace("-modal", "");
  const el = document.getElementById(`${suffix}-message`);
  if (!el) return;
  el.textContent = text;
  el.hidden = !text;
  el.classList.toggle("error", isError);
}

function clearModalMessages() {
  showModalMessage("login-modal", "");
  showModalMessage("register-modal", "");
}

function updateCaptchaLabel() {
  const label = document.getElementById("captcha-label");
  if (label) {
    label.textContent = t("field.captcha", { question: captchaQuestionText });
  }
}

async function api(path, options = {}) {
  const headers = { "Content-Type": "application/json", ...(options.headers || {}) };
  const token = localStorage.getItem("mpulse_token");
  if (token && !path.includes("/auth/")) {
    headers.Authorization = `Bearer ${token}`;
  }
  const res = await fetch(`${API}${path}`, { ...options, headers });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(humanError(data.error));
  return data;
}

async function loadCaptcha() {
  const res = await fetch(`${API}/v1/auth/captcha`);
  const data = await res.json();
  captchaId = data.id;
  captchaQuestionText = `${data.question} = ?`;
  updateCaptchaLabel();
  const answerInput = document.querySelector("#register-form input[name=captcha_answer]");
  if (answerInput) answerInput.value = "";
}

function renderBetaBanner() {
  const banner = document.getElementById("beta-banner");
  if (!banner) return;
  if (!betaConfig.selfService) {
    banner.hidden = true;
    return;
  }
  banner.hidden = false;
  banner.textContent = t("beta.banner");
}

function renderPlansList() {
  /* Public tariffs list hidden on client site; plans load for subscribe section only. */
}

async function loadPlans() {
  const data = await api("/v1/plans/public");
  publicPlans = data.plans ?? [];
  betaConfig = data.beta ?? { selfService: false, maxDevices: 10 };
  renderPlansList();
  renderBetaBanner();
  renderSubscribePlans();
}

function renderSubscribePlans() {
  const section = document.getElementById("subscribe-section");
  const root = document.getElementById("subscribe-plans");
  const devicesInput = document.getElementById("subscribe-devices");
  if (!section || !root || !devicesInput) return;

  if (!betaConfig.selfService || !localStorage.getItem("mpulse_token")) {
    section.hidden = true;
    return;
  }

  section.hidden = false;
  devicesInput.max = String(betaConfig.maxDevices);
  const hint = section.querySelector(".subscribe-devices-hint");
  if (hint) {
    hint.textContent = t("subscribe.devicesHint", { max: betaConfig.maxDevices });
  }

  if (!selectedPlanId && publicPlans.length) {
    selectedPlanId = publicPlans[0].id;
  }

  root.innerHTML = publicPlans
    .map(
      (p) => `<button type="button" class="plan plan-selectable${p.id === selectedPlanId ? " selected" : ""}" data-plan-id="${p.id}">
        <strong>${p.name}</strong>
        <div>${p.tier}</div>
        <div class="plan-price">${(p.priceCents / 100).toFixed(0)} ${p.currency}</div>
        <div class="plan-meta">${t("plans.duration", { days: p.durationDays, devices: p.maxDevices })}</div>
      </button>`,
    )
    .join("");

  root.querySelectorAll("[data-plan-id]").forEach((btn) => {
    btn.addEventListener("click", () => {
      selectedPlanId = btn.dataset.planId;
      renderSubscribePlans();
    });
  });

  if (!devicesInput.value || Number(devicesInput.value) < 1) {
    devicesInput.value = String(Math.max(1, dashboardDeviceLimit));
  }
}

function showSubscribeMessage(text, isError = false) {
  const el = document.getElementById("subscribe-message");
  if (!el) return;
  el.textContent = text;
  el.hidden = !text;
  el.classList.toggle("error", isError);
}

function formatSubscriptionEnd(value) {
  if (value == null || value === "") return "∞";
  return formatClientDateTime(value);
}

function renderActivationCode() {
  if (!lastActivationCode) return;
  const el = document.getElementById("activation-code");
  el.textContent = `${t("auth.codeLine", { code: lastActivationCode })}\n${t("auth.codeExpires", {
    date: formatClientDateTime(lastActivationExpiresAt),
  })}`;
}

function showGuestAuth() {
  document.getElementById("auth-actions").hidden = false;
  document.getElementById("auth-hint").hidden = false;
  document.getElementById("dashboard").hidden = true;
  renderSubscribePlans();
}

function escHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/"/g, "&quot;");
}

function deviceLabel(device) {
  if (device.label) return device.label;
  if (device.os && device.appVersion) return `${device.os} · ${device.appVersion}`;
  if (device.os) return device.os;
  return device.hwid.slice(0, 12) + "…";
}

function renderDevices(devices, deviceLimit) {
  const summary = document.getElementById("devices-summary");
  const list = document.getElementById("devices-list");
  if (!summary || !list) return;

  summary.textContent = t("devices.summary", {
    count: devices.length,
    limit: deviceLimit,
  });

  if (!devices.length) {
    list.innerHTML = `<p class="devices-empty">${t("devices.empty")}</p>`;
    return;
  }

  list.innerHTML = `<table class="devices-table">
    <thead>
      <tr>
        <th>${t("devices.col.name")}</th>
        <th>${t("devices.col.lastSeen")}</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      ${devices
        .map(
          (device) => `<tr>
            <td>
              <strong>${escHtml(deviceLabel(device))}</strong>
              <div class="devices-meta">${escHtml(device.hwid)}</div>
            </td>
            <td>${escHtml(formatClientDateTime(device.lastSeenAt))}</td>
            <td class="devices-actions">
              <button type="button" class="secondary-btn device-delete-btn" data-id="${escHtml(device.id)}">${t("devices.delete")}</button>
            </td>
          </tr>`,
        )
        .join("")}
    </tbody>
  </table>`;

  list.querySelectorAll(".device-delete-btn").forEach((btn) => {
    btn.addEventListener("click", async () => {
      if (!confirm(t("devices.deleteConfirm"))) return;
      try {
        await api(`/v1/account/devices/${btn.dataset.id}`, { method: "DELETE" });
        await refreshDashboard();
      } catch (err) {
        alert(err.message || t("error.generic"));
      }
    });
  });
}

async function renderLogs() {
  const list = document.getElementById("logs-list");
  if (!list) return;
  try {
    const { logs } = await api("/v1/account/logs");
    if (!logs?.length) {
      list.innerHTML = `<p class="logs-empty">${t("logs.empty")}</p>`;
      return;
    }
    list.innerHTML = `<table class="logs-table">
      <thead>
        <tr>
          <th>${t("logs.col.date")}</th>
          <th>${t("logs.col.file")}</th>
          <th>${t("logs.col.hwid")}</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        ${logs
          .map(
            (row) => `<tr>
              <td>${escHtml(formatClientDateTime(row.created_at))}</td>
              <td><code>${escHtml(row.filename)}</code></td>
              <td><code>${escHtml(row.hwid)}</code></td>
              <td>
                <a class="secondary-btn" href="${escHtml(window.MPULSE_API + "/v1/account/logs/" + row.id + "/download")}" download>${t("logs.download")}</a>
              </td>
            </tr>`,
          )
          .join("")}
      </tbody>
    </table>`;
    list.querySelectorAll("a.secondary-btn").forEach((link) => {
      link.addEventListener("click", async (e) => {
        e.preventDefault();
        const token = localStorage.getItem("mpulse_token");
        if (!token) return;
        const href = link.getAttribute("href");
        const response = await fetch(href, {
          headers: { Authorization: `Bearer ${token}` },
        });
        if (!response.ok) {
          alert(t("error.generic"));
          return;
        }
        const blob = await response.blob();
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = link.closest("tr")?.querySelector("code")?.textContent ?? "log.zip";
        a.click();
        URL.revokeObjectURL(url);
      });
    });
  } catch {
    list.innerHTML = `<p class="logs-empty">${t("logs.loadError")}</p>`;
  }
}

async function renderStorageSection() {
  const toggle = document.getElementById("storage-shared-toggle");
  const list = document.getElementById("storage-backups-list");
  if (!toggle || !list) return;
  try {
    const mode = await api("/v1/account/storage-mode");
    toggle.checked = mode.shared !== false;
    toggle.onchange = async () => {
      try {
        await api("/v1/account/storage-mode", {
          method: "PUT",
          body: JSON.stringify({ shared: toggle.checked }),
        });
      } catch (err) {
        alert(err.message || t("error.generic"));
        toggle.checked = !toggle.checked;
      }
    };
  } catch {
    /* ignore */
  }
  try {
    const { backups } = await api("/v1/account/storage-backups");
    if (!backups?.length) {
      list.innerHTML = `<p class="storage-empty">${t("storage.empty")}</p>`;
      return;
    }
    list.innerHTML = `<table class="storage-table">
      <thead><tr><th>${t("storage.col.hwid")}</th><th>${t("storage.col.updated")}</th></tr></thead>
      <tbody>
        ${backups
          .map(
            (row) => `<tr>
              <td><code>${escHtml(row.hwid)}</code></td>
              <td>${escHtml(formatClientDateTime(row.updated_at))}</td>
            </tr>`,
          )
          .join("")}
      </tbody>
    </table>`;
  } catch {
    list.innerHTML = `<p class="storage-empty">${t("storage.loadError")}</p>`;
  }
}

function showDashboard(user, subscription, devices = [], deviceLimit = 1) {
  document.getElementById("auth-actions").hidden = true;
  document.getElementById("auth-hint").hidden = true;
  closeAllModals();
  document.getElementById("dashboard").hidden = false;
  document.getElementById("user-email").textContent = user.email;
  document.getElementById("user-nickname").textContent = user.nickname ? `@${user.nickname}` : "";
  dashboardDeviceLimit = deviceLimit;
  if (subscription?.planId) {
    selectedPlanId = subscription.planId;
  }
  document.getElementById("subscription-info").textContent = subscription
    ? t("auth.subscriptionActive", {
        name: subscription.plan.name,
        tier: subscription.plan.tier,
        date: formatSubscriptionEnd(subscription.endsAt),
      })
    : betaConfig.selfService
      ? t("auth.subscriptionNoneBeta")
      : t("auth.subscriptionNone");
  renderDevices(devices, deviceLimit);
  void renderStorageSection();
  void renderLogs();
  renderSubscribePlans();
  showSubscribeMessage("");
}

async function refreshDashboard() {
  const token = localStorage.getItem("mpulse_token");
  if (!token) {
    showGuestAuth();
    return;
  }
  try {
    const me = await api("/v1/account/me");
    if (me.beta) {
      betaConfig = me.beta;
      renderBetaBanner();
    }
    showDashboard(me.user, me.subscription, me.devices ?? [], me.deviceLimit ?? 1);
  } catch {
    localStorage.removeItem("mpulse_token");
    localStorage.removeItem("mpulse_refresh");
    showGuestAuth();
  }
}

function bindApp() {
  document.getElementById("open-login").addEventListener("click", () => {
    clearModalMessages();
    openModal("login-modal");
  });

  document.getElementById("open-register").addEventListener("click", () => {
    clearModalMessages();
    loadCaptcha().catch(console.error);
    openModal("register-modal");
  });

  document.querySelectorAll("[data-close]").forEach((el) => {
    el.addEventListener("click", () => closeModal(el.dataset.close));
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeAllModals();
  });

  document.getElementById("captcha-refresh")?.addEventListener("click", () => {
    loadCaptcha().catch(console.error);
  });

  document.getElementById("login-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    try {
      const data = await api("/v1/auth/login", {
        method: "POST",
        body: JSON.stringify({ email: fd.get("email"), password: fd.get("password") }),
      });
      localStorage.setItem("mpulse_token", data.access_token);
      localStorage.setItem("mpulse_refresh", data.refresh_token);
      closeModal("login-modal");
      await refreshDashboard();
    } catch (err) {
      showModalMessage("login-modal", err.message || t("error.loginFailed"), true);
    }
  });

  document.getElementById("register-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    if (fd.get("password") !== fd.get("password_confirm")) {
      showModalMessage("register-modal", t("error.password_mismatch"), true);
      return;
    }
    try {
      await api("/v1/auth/register", {
        method: "POST",
        body: JSON.stringify({
          email: fd.get("email"),
          nickname: String(fd.get("nickname")).toLowerCase(),
          password: fd.get("password"),
          password_confirm: fd.get("password_confirm"),
          captcha_id: captchaId,
          captcha_answer: fd.get("captcha_answer"),
        }),
      });
      closeModal("register-modal");
      e.target.reset();
      document.querySelector("#login-form input[name=email]").value = fd.get("email");
      openModal("login-modal");
      showModalMessage("login-modal", t("auth.registerSuccess"));
    } catch (err) {
      showModalMessage("register-modal", err.message || t("error.registerFailed"), true);
      loadCaptcha().catch(console.error);
    }
  });

  document.getElementById("subscribe-btn")?.addEventListener("click", async () => {
    const devicesInput = document.getElementById("subscribe-devices");
    const deviceCount = Number(devicesInput?.value ?? 1);
    if (!selectedPlanId) {
      showSubscribeMessage(t("error.plan_required"), true);
      return;
    }
    showSubscribeMessage("");
    try {
      await api("/v1/account/subscribe", {
        method: "POST",
        body: JSON.stringify({ planId: selectedPlanId, deviceCount }),
      });
      showSubscribeMessage(t("subscribe.success"));
      await refreshDashboard();
    } catch (err) {
      showSubscribeMessage(err.message || t("error.generic"), true);
    }
  });

  document.getElementById("gen-code").addEventListener("click", async () => {
    const data = await api("/v1/account/activation-code", {
      method: "POST",
      body: "{}",
    });
    lastActivationCode = data.code;
    lastActivationExpiresAt = data.expires_at;
    renderActivationCode();
  });

  document.getElementById("logout").addEventListener("click", () => {
    localStorage.removeItem("mpulse_token");
    localStorage.removeItem("mpulse_refresh");
    location.reload();
  });

  document.addEventListener("mpulse:locale", () => {
    loadPlans().catch(console.error);
    refreshDashboard().catch(() => {});
    updateCaptchaLabel();
    renderActivationCode();
  });
  loadPlans().catch(console.error);
  refreshDashboard().catch(() => {});
}

document.addEventListener("mpulse:ready", bindApp);
