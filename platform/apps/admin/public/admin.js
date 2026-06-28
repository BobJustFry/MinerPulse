const API = window.MPULSE_API || "http://localhost:3001";
let token = localStorage.getItem("mpulse_admin_token");

async function api(path, options = {}) {
  const headers = { "Content-Type": "application/json", ...(options.headers || {}) };
  if (token) headers.Authorization = `Bearer ${token}`;
  const res = await fetch(`${API}${path}`, { ...options, headers });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

function showPanel() {
  document.getElementById("login").hidden = true;
  document.getElementById("panel").hidden = false;
}

async function loadUsers() {
  const { users } = await api("/v1/admin/users");
  document.getElementById("content").innerHTML = `<table><tr><th>Email</th><th>Devices</th><th>Sub</th><th></th></tr>${users
    .map(
      (u) => `<tr><td>${u.email}</td><td>${u._count.devices}</td><td>${u.subscriptions[0]?.plan?.name ?? "-"}</td><td><button data-user="${u.id}" class="code-btn">Код</button></td></tr>`,
    )
    .join("")}</table>`;
  document.querySelectorAll(".code-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      const data = await api("/v1/admin/activation-codes", {
        method: "POST",
        body: JSON.stringify({ userId: btn.dataset.user }),
      });
      alert(`Код: ${data.code}\nДо: ${data.expires_at}`);
    }),
  );
}

async function loadPlans() {
  const { plans } = await api("/v1/admin/plans");
  document.getElementById("content").innerHTML = `<table><tr><th>Name</th><th>Tier</th><th>Price</th><th>Active</th></tr>${plans
    .map((p) => `<tr><td>${p.name}</td><td>${p.tier}</td><td>${p.priceCents / 100} ${p.currency}</td><td>${p.active}</td></tr>`)
    .join("")}</table>`;
}

async function loadSubsForm() {
  const [{ users }, { plans }] = await Promise.all([api("/v1/admin/users"), api("/v1/admin/plans")]);
  document.getElementById("content").innerHTML = `
    <form id="sub-form">
      <label>User<select name="userId">${users.map((u) => `<option value="${u.id}">${u.email}</option>`).join("")}</select></label>
      <label>Plan<select name="planId">${plans.map((p) => `<option value="${p.id}">${p.name}</option>`).join("")}</select></label>
      <button type="submit">Создать подписку</button>
    </form>`;
  document.getElementById("sub-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    await api("/v1/admin/subscriptions", {
      method: "POST",
      body: JSON.stringify({ userId: fd.get("userId"), planId: fd.get("planId") }),
    });
    alert("Подписка создана");
  });
}

async function loadAudit() {
  const { logs } = await api("/v1/admin/audit");
  document.getElementById("content").innerHTML = `<table><tr><th>When</th><th>Admin</th><th>Action</th><th>Entity</th></tr>${logs
    .map((l) => `<tr><td>${l.createdAt}</td><td>${l.admin.email}</td><td>${l.action}</td><td>${l.entity}</td></tr>`)
    .join("")}</table>`;
}

document.querySelectorAll("[data-tab]").forEach((btn) =>
  btn.addEventListener("click", () => {
    const tab = btn.dataset.tab;
    if (tab === "users") loadUsers();
    if (tab === "plans") loadPlans();
    if (tab === "subs") loadSubsForm();
    if (tab === "audit") loadAudit();
  }),
);

document.getElementById("login-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  const data = await api("/v1/admin/auth/login", {
    method: "POST",
    body: JSON.stringify({ email: fd.get("email"), password: fd.get("password") }),
  });
  token = data.access_token;
  localStorage.setItem("mpulse_admin_token", token);
  showPanel();
  loadUsers();
});

document.getElementById("logout").addEventListener("click", () => {
  localStorage.removeItem("mpulse_admin_token");
  location.reload();
});

if (token) {
  showPanel();
  loadUsers().catch(() => {
    localStorage.removeItem("mpulse_admin_token");
    location.reload();
  });
}
