const API = window.MPULSE_API || "http://localhost:3001";

const ERROR_RU = {
  email_taken: "Email уже зарегистрирован",
  nickname_taken: "Ник уже занят",
  captcha_failed: "Неверный ответ капчи",
  password_mismatch: "Пароли не совпадают",
  invalid_credentials: "Неверный email или пароль",
  validation_failed: "Проверьте поля формы",
};

function humanError(code) {
  return ERROR_RU[code] || code || "Ошибка";
}

let captchaId = null;

function adminPortalUrl() {
  const host = window.location.hostname;
  if (host === "localhost" || host === "127.0.0.1") {
    return "https://admin.mpulse.bob4.fun";
  }
  if (host.startsWith("admin.")) {
    return window.location.origin;
  }
  return `${window.location.protocol}//admin.${host}`;
}

function setAuthTab(tab) {
  document.querySelectorAll(".auth-tab").forEach((btn) => {
    const active = btn.dataset.tab === tab;
    btn.classList.toggle("active", active);
    btn.setAttribute("aria-selected", active ? "true" : "false");
  });
  document.getElementById("login-form").hidden = tab !== "login";
  document.getElementById("login-form").classList.toggle("active", tab === "login");
  document.getElementById("register-form").hidden = tab !== "register";
  document.getElementById("register-form").classList.toggle("active", tab === "register");
  document.getElementById("auth-message").hidden = true;
  if (tab === "register") {
    loadCaptcha().catch(console.error);
  }
}

function showAuthMessage(text, isError = false) {
  const el = document.getElementById("auth-message");
  el.textContent = text;
  el.hidden = false;
  el.classList.toggle("error", isError);
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
  document.getElementById("captcha-question").textContent = `${data.question} = ?`;
  const answerInput = document.querySelector("#register-form input[name=captcha_answer]");
  if (answerInput) answerInput.value = "";
}

async function loadPlans() {
  const { plans } = await api("/v1/plans/public");
  const root = document.getElementById("plans-list");
  root.innerHTML = plans
    .map(
      (p) => `<div class="plan"><strong>${p.name}</strong><div>${p.tier}</div><div>${(p.priceCents / 100).toFixed(0)} ${p.currency}</div><div>${p.durationDays} дн · ${p.maxDevices} устр.</div></div>`,
    )
    .join("");
}

function showDashboard(user, subscription) {
  document.getElementById("auth-forms").hidden = true;
  document.getElementById("dashboard").hidden = false;
  document.getElementById("user-email").textContent = user.email;
  document.getElementById("user-nickname").textContent = user.nickname ? `@${user.nickname}` : "";
  document.getElementById("subscription-info").textContent = subscription
    ? `Подписка: ${subscription.plan.name} (${subscription.plan.tier}) до ${subscription.endsAt ?? "∞"}`
    : "Нет активной подписки — обратитесь к администратору.";
}

async function refreshDashboard() {
  const token = localStorage.getItem("mpulse_token");
  if (!token) return;
  const me = await api("/v1/account/me");
  showDashboard(me.user, me.subscription);
}

document.querySelectorAll(".auth-tab").forEach((btn) => {
  btn.addEventListener("click", () => setAuthTab(btn.dataset.tab));
});

const adminLink = document.getElementById("admin-link");
if (adminLink) {
  adminLink.href = adminPortalUrl();
  adminLink.textContent = adminPortalUrl().replace(/^https?:\/\//, "");
}

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
    await refreshDashboard();
  } catch (err) {
    showAuthMessage(err.message || "Ошибка входа", true);
  }
});

document.getElementById("register-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  if (fd.get("password") !== fd.get("password_confirm")) {
    showAuthMessage(ERROR_RU.password_mismatch, true);
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
    setAuthTab("login");
    showAuthMessage("Аккаунт создан. Войдите с тем же email и паролем.");
    document.querySelector("#login-form input[name=email]").value = fd.get("email");
    document.querySelector("#login-form input[name=password]").focus();
  } catch (err) {
    showAuthMessage(err.message || "Ошибка регистрации", true);
    loadCaptcha().catch(console.error);
  }
});

document.getElementById("gen-code").addEventListener("click", async () => {
  const data = await api("/v1/account/activation-code", {
    method: "POST",
    body: "{}",
  });
  document.getElementById("activation-code").textContent = `Код: ${data.code}\nДействует до: ${data.expires_at}`;
});

document.getElementById("logout").addEventListener("click", () => {
  localStorage.removeItem("mpulse_token");
  localStorage.removeItem("mpulse_refresh");
  location.reload();
});

loadPlans().catch(console.error);
refreshDashboard().catch(() => {});
