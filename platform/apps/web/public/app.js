const API = window.MPULSE_API || "http://localhost:3001";

async function api(path, options = {}) {
  const headers = { "Content-Type": "application/json", ...(options.headers || {}) };
  const token = localStorage.getItem("mpulse_token");
  if (token && !path.includes("/auth/")) {
    headers.Authorization = `Bearer ${token}`;
  }
  const res = await fetch(`${API}${path}`, { ...options, headers });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
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

function showDashboard(email, subscription) {
  document.getElementById("auth-forms").hidden = true;
  document.getElementById("dashboard").hidden = false;
  document.getElementById("user-email").textContent = email;
  document.getElementById("subscription-info").textContent = subscription
    ? `Подписка: ${subscription.plan.name} (${subscription.plan.tier}) до ${subscription.endsAt ?? "∞"}`
    : "Нет активной подписки — обратитесь к администратору.";
}

async function refreshDashboard() {
  const token = localStorage.getItem("mpulse_token");
  if (!token) return;
  const me = await api("/v1/account/me");
  showDashboard(me.user.email, me.subscription);
}

document.getElementById("login-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  const data = await api("/v1/auth/login", {
    method: "POST",
    body: JSON.stringify({ email: fd.get("email"), password: fd.get("password") }),
  });
  localStorage.setItem("mpulse_token", data.access_token);
  localStorage.setItem("mpulse_refresh", data.refresh_token);
  await refreshDashboard();
});

document.getElementById("register-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  await api("/v1/auth/register", {
    method: "POST",
    body: JSON.stringify({ email: fd.get("email"), password: fd.get("password") }),
  });
  alert("Аккаунт создан. Войдите.");
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
