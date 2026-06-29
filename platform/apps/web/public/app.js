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
  document.getElementById("auth-actions").hidden = true;
  closeAllModals();
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
    showModalMessage("login-modal", err.message || "Ошибка входа", true);
  }
});

document.getElementById("register-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  if (fd.get("password") !== fd.get("password_confirm")) {
    showModalMessage("register-modal", ERROR_RU.password_mismatch, true);
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
    showModalMessage("login-modal", "Аккаунт создан. Войдите с тем же email и паролем.");
  } catch (err) {
    showModalMessage("register-modal", err.message || "Ошибка регистрации", true);
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
