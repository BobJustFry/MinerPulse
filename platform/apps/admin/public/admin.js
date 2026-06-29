const API = window.MPULSE_API || "http://localhost:3001";
let token = localStorage.getItem("mpulse_admin_token");

function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/"/g, "&quot;");
}

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

function showMsg(text, isError = false) {
  const el = document.getElementById("admin-msg");
  if (!el) return;
  el.textContent = text;
  el.hidden = !text;
  el.classList.toggle("error", isError);
}

async function loadUsers() {
  const { users } = await api("/v1/admin/users");
  document.getElementById("content").innerHTML = `
    <div class="toolbar">
      <button type="button" id="user-add">+ Добавить клиента</button>
    </div>
    <div id="user-form-wrap" hidden></div>
    <table>
      <tr><th>Email</th><th>Ник</th><th>Devices</th><th>Подписка</th><th></th></tr>
      ${users
        .map(
          (u) => `<tr>
            <td>${esc(u.email)}</td>
            <td>${esc(u.nickname)}</td>
            <td>${u._count.devices}</td>
            <td>${esc(u.subscriptions[0]?.plan?.name ?? "—")}</td>
            <td class="actions">
              <button type="button" class="code-btn" data-user="${u.id}">Код</button>
              <button type="button" class="edit-user-btn" data-id="${u.id}">Изм.</button>
              <button type="button" class="del-user-btn" data-id="${u.id}">Удал.</button>
            </td>
          </tr>`,
        )
        .join("")}
    </table>`;

  document.getElementById("user-add").addEventListener("click", () => showUserForm());
  document.querySelectorAll(".code-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      const data = await api("/v1/admin/activation-codes", {
        method: "POST",
        body: JSON.stringify({ userId: btn.dataset.user }),
      });
      alert(`Код: ${data.code}\nДо: ${data.expires_at}`);
    }),
  );
  document.querySelectorAll(".edit-user-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      const { user } = await api(`/v1/admin/users/${btn.dataset.id}`);
      showUserForm(btn.dataset.id, user);
    }),
  );
  document.querySelectorAll(".del-user-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      if (!confirm("Удалить клиента и все связанные данные?")) return;
      await api(`/v1/admin/users/${btn.dataset.id}`, { method: "DELETE" });
      loadUsers();
    }),
  );
}

function showUserForm(id = null, user = null) {
  const wrap = document.getElementById("user-form-wrap");
  wrap.hidden = false;
  const devicesHtml =
    id && user?.devices?.length
      ? `<div class="device-list">
          <h4>Устройства</h4>
          <table>
            <tr><th>HWID</th><th>ОС</th><th>Версия ОС</th><th>Версия</th><th>Билд</th><th>Последний вход</th></tr>
            ${user.devices
              .map(
                (d) => `<tr>
                  <td><code>${esc(d.hwid)}</code></td>
                  <td>${esc(d.os ?? "—")}</td>
                  <td>${esc(d.osVersion ?? "—")}</td>
                  <td>${esc(d.appVersion ?? "—")}</td>
                  <td>${esc(d.appBuild ?? "—")}</td>
                  <td>${esc(d.lastSeenAt ? new Date(d.lastSeenAt).toLocaleString("ru-RU") : "—")}</td>
                </tr>`,
              )
              .join("")}
          </table>
        </div>`
      : id
        ? `<p class="muted">Устройств пока нет.</p>`
        : "";
  wrap.innerHTML = `
    <form id="user-form" class="inline-form">
      <h3>${id ? "Редактировать клиента" : "Новый клиент"}</h3>
      <label>Email<input name="email" type="email" value="${esc(user?.email ?? "")}" required /></label>
      <label>Ник<input name="nickname" pattern="[A-Za-z0-9_]{3,32}" value="${esc(user?.nickname ?? "")}" required /></label>
      <label>Пароль<input name="password" type="password" minlength="8" ${id ? "" : "required"} placeholder="${id ? "оставить пустым — без смены" : ""}" /></label>
      ${devicesHtml}
      <div class="form-actions">
        <button type="submit">Сохранить</button>
        <button type="button" id="user-form-cancel">Отмена</button>
      </div>
    </form>`;
  document.getElementById("user-form-cancel").addEventListener("click", () => {
    wrap.hidden = true;
  });
  document.getElementById("user-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    const body = {
      email: fd.get("email"),
      nickname: fd.get("nickname"),
    };
    if (fd.get("password")) body.password = fd.get("password");
    try {
      if (id) {
        await api(`/v1/admin/users/${id}`, { method: "PATCH", body: JSON.stringify(body) });
      } else {
        await api("/v1/admin/users", { method: "POST", body: JSON.stringify(body) });
      }
      wrap.hidden = true;
      loadUsers();
    } catch (err) {
      showMsg(err.message, true);
    }
  });
}

async function loadPlans() {
  const { plans } = await api("/v1/admin/plans");
  document.getElementById("content").innerHTML = `
    <div class="toolbar">
      <button type="button" id="plan-add">+ Добавить тариф</button>
    </div>
    <div id="plan-form-wrap" hidden></div>
    <table>
      <tr><th>Name</th><th>Slug</th><th>Tier</th><th>Price</th><th>Days</th><th>Devices</th><th>Active</th><th></th></tr>
      ${plans
        .map(
          (p) => `<tr>
            <td>${esc(p.name)}</td>
            <td>${esc(p.slug)}</td>
            <td>${esc(p.tier)}</td>
            <td>${p.priceCents / 100} ${esc(p.currency)}</td>
            <td>${p.durationDays}</td>
            <td>${p.maxDevices}</td>
            <td>${p.active ? "да" : "нет"}</td>
            <td class="actions">
              <button type="button" class="edit-plan-btn" data-id="${p.id}">Изм.</button>
              <button type="button" class="del-plan-btn" data-id="${p.id}">Удал.</button>
            </td>
          </tr>`,
        )
        .join("")}
    </table>`;

  document.getElementById("plan-add").addEventListener("click", () => showPlanForm());
  document.querySelectorAll(".edit-plan-btn").forEach((btn) =>
    btn.addEventListener("click", () => showPlanForm(btn.dataset.id, plans.find((p) => p.id === btn.dataset.id))),
  );
  document.querySelectorAll(".del-plan-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      if (!confirm("Удалить тариф? (если есть подписки — будет деактивирован)")) return;
      const data = await api(`/v1/admin/plans/${btn.dataset.id}`, { method: "DELETE" });
      alert(data.deactivated ? "Тариф деактивирован (есть подписки)" : "Тариф удалён");
      loadPlans();
    }),
  );
}

function showPlanForm(id = null, plan = null) {
  const wrap = document.getElementById("plan-form-wrap");
  wrap.hidden = false;
  wrap.innerHTML = `
    <form id="plan-form" class="inline-form">
      <h3>${id ? "Редактировать тариф" : "Новый тариф"}</h3>
      <label>Slug<input name="slug" value="${esc(plan?.slug ?? "")}" ${id ? "readonly" : "required"} /></label>
      <label>Название<input name="name" value="${esc(plan?.name ?? "")}" required /></label>
      <label>Tier
        <select name="tier">
          ${["FREE", "CLIENT", "SERVICE"]
            .map((t) => `<option value="${t}" ${plan?.tier === t ? "selected" : ""}>${t}</option>`)
            .join("")}
        </select>
      </label>
      <label>Цена (коп.)<input name="priceCents" type="number" min="0" value="${plan?.priceCents ?? 99000}" required /></label>
      <label>Валюта<input name="currency" value="${esc(plan?.currency ?? "RUB")}" /></label>
      <label>Дней<input name="durationDays" type="number" min="1" value="${plan?.durationDays ?? 30}" required /></label>
      <label>Устройств<input name="maxDevices" type="number" min="1" value="${plan?.maxDevices ?? 1}" required /></label>
      <label>Сортировка<input name="sortOrder" type="number" value="${plan?.sortOrder ?? 0}" /></label>
      <label><input type="checkbox" name="active" ${plan?.active !== false ? "checked" : ""} /> Активен</label>
      <div class="form-actions">
        <button type="submit">Сохранить</button>
        <button type="button" id="plan-form-cancel">Отмена</button>
      </div>
    </form>`;
  document.getElementById("plan-form-cancel").addEventListener("click", () => {
    wrap.hidden = true;
  });
  document.getElementById("plan-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    const body = {
      slug: fd.get("slug"),
      name: fd.get("name"),
      tier: fd.get("tier"),
      priceCents: Number(fd.get("priceCents")),
      currency: fd.get("currency"),
      durationDays: Number(fd.get("durationDays")),
      maxDevices: Number(fd.get("maxDevices")),
      sortOrder: Number(fd.get("sortOrder")),
      active: fd.get("active") === "on",
    };
    try {
      if (id) {
        delete body.slug;
        await api(`/v1/admin/plans/${id}`, { method: "PATCH", body: JSON.stringify(body) });
      } else {
        await api("/v1/admin/plans", { method: "POST", body: JSON.stringify(body) });
      }
      wrap.hidden = true;
      loadPlans();
    } catch (err) {
      showMsg(err.message, true);
    }
  });
}

async function loadSubsForm() {
  const [{ users }, { plans }] = await Promise.all([api("/v1/admin/users"), api("/v1/admin/plans")]);
  document.getElementById("content").innerHTML = `
    <form id="sub-form">
      <label>Клиент<select name="userId">${users.map((u) => `<option value="${u.id}">${esc(u.email)} (${esc(u.nickname)})</option>`).join("")}</select></label>
      <label>Тариф<select name="planId">${plans.map((p) => `<option value="${p.id}">${esc(p.name)}</option>`).join("")}</select></label>
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
    .map((l) => `<tr><td>${esc(l.createdAt)}</td><td>${esc(l.admin.username ?? l.admin.email ?? "-")}</td><td>${esc(l.action)}</td><td>${esc(l.entity)}</td></tr>`)
    .join("")}</table>`;
}

document.querySelectorAll("[data-tab]").forEach((btn) =>
  btn.addEventListener("click", () => {
    showMsg("");
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
  try {
    const data = await api("/v1/admin/auth/login", {
      method: "POST",
      body: JSON.stringify({ username: fd.get("username"), password: fd.get("password") }),
    });
    token = data.access_token;
    localStorage.setItem("mpulse_admin_token", token);
    showPanel();
    loadUsers();
  } catch (err) {
    alert(err.message);
  }
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
