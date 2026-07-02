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
      <tr><th>Email</th><th>Ник</th><th>Устройства</th><th>Подписка</th><th></th></tr>
      ${users
        .map(
          (u) => `<tr>
            <td>${esc(u.email)}</td>
            <td>${esc(u.nickname)}</td>
            <td>${u.deviceCount ?? u._count.devices}/${u.deviceLimit ?? u._count.devices}</td>
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
    btn.addEventListener("click", () => openUserEdit(btn.dataset.id)),
  );
  document.querySelectorAll(".del-user-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      if (!confirm("Удалить клиента и все связанные данные?")) return;
      await api(`/v1/admin/users/${btn.dataset.id}`, { method: "DELETE" });
      loadUsers();
    }),
  );
}

async function openUserEdit(id) {
  const { user } = await api(`/v1/admin/users/${id}`);
  showUserForm(id, user);
}

function bindDeviceActions(userId) {
  document.querySelectorAll(".edit-device-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      const label = prompt("Метка устройства (пусто — без метки):", btn.dataset.label ?? "");
      if (label === null) return;
      try {
        await api(`/v1/admin/devices/${btn.dataset.id}`, {
          method: "PATCH",
          body: JSON.stringify({ label: label.trim() || null }),
        });
        await openUserEdit(userId);
        showMsg("Устройство обновлено");
      } catch (err) {
        showMsg(err.message, true);
      }
    }),
  );

  document.querySelectorAll(".del-device-btn").forEach((btn) =>
    btn.addEventListener("click", async () => {
      if (!confirm("Удалить устройство? Клиенту потребуется повторная активация на этом ПК.")) return;
      try {
        await api(`/v1/admin/devices/${btn.dataset.id}`, { method: "DELETE" });
        await openUserEdit(userId);
        loadUsers();
        showMsg("Устройство удалено");
      } catch (err) {
        showMsg(err.message, true);
      }
    }),
  );

  const addForm = document.getElementById("device-add-form");
  if (addForm) {
    addForm.addEventListener("submit", async (e) => {
      e.preventDefault();
      const fd = new FormData(addForm);
      const hwid = String(fd.get("hwid") ?? "").trim();
      const label = String(fd.get("label") ?? "").trim();
      if (hwid.length < 8) {
        showMsg("HWID не короче 8 символов", true);
        return;
      }
      try {
        await api(`/v1/admin/users/${userId}/devices`, {
          method: "POST",
          body: JSON.stringify({ hwid, label: label || null }),
        });
        await openUserEdit(userId);
        loadUsers();
        showMsg("Устройство добавлено");
      } catch (err) {
        showMsg(err.message === "device_limit" ? "Достигнут лимит устройств" : err.message, true);
      }
    });
  }
}

function showUserForm(id = null, user = null) {
  const wrap = document.getElementById("user-form-wrap");
  wrap.hidden = false;
  const planMax = user?.devicePlanMax ?? user?.subscriptions?.[0]?.plan?.maxDevices ?? 1;
  const deviceLimit = user?.deviceLimit ?? planMax;
  const deviceCount = user?.deviceCount ?? user?.devices?.length ?? user?._count?.devices ?? 0;
  const devicesHtml = id
    ? `<div class="device-list">
          <h4>Устройства (${deviceCount}/${deviceLimit})</h4>
          ${
            user?.devices?.length
              ? `<table>
            <tr><th>Метка</th><th>HWID</th><th>ОС</th><th>Версия ОС</th><th>App</th><th>Билд</th><th>Последний вход</th><th></th></tr>
            ${user.devices
              .map(
                (d) => `<tr>
                  <td>${esc(d.label ?? "—")}</td>
                  <td><code>${esc(d.hwid)}</code></td>
                  <td>${esc(d.os ?? "—")}</td>
                  <td>${esc(d.osVersion ?? "—")}</td>
                  <td>${esc(d.appVersion ?? "—")}</td>
                  <td>${esc(d.appBuild ?? "—")}</td>
                  <td>${esc(d.lastSeenAt ? new Date(d.lastSeenAt).toLocaleString("ru-RU") : "—")}</td>
                  <td class="actions">
                    <button type="button" class="edit-device-btn" data-id="${d.id}" data-label="${esc(d.label ?? "")}">Метка</button>
                    <button type="button" class="del-device-btn" data-id="${d.id}">Удал.</button>
                  </td>
                </tr>`,
              )
              .join("")}
          </table>`
              : `<p class="muted">Устройств пока нет.</p>`
          }
          <form id="device-add-form" class="device-add-form">
            <h4>Добавить устройство</h4>
            <label>HWID<input name="hwid" minlength="8" required placeholder="мин. 8 символов" /></label>
            <label>Метка<input name="label" placeholder="необязательно" /></label>
            <button type="submit">Добавить</button>
          </form>
        </div>`
    : "";
  wrap.innerHTML = `
    <form id="user-form" class="inline-form">
      <h3>${id ? "Редактировать клиента" : "Новый клиент"}</h3>
      <label>Email<input name="email" type="email" value="${esc(user?.email ?? "")}" required /></label>
      <label>Ник<input name="nickname" pattern="[A-Za-z0-9_]{3,32}" value="${esc(user?.nickname ?? "")}" required /></label>
      <label>Пароль<input name="password" type="password" minlength="8" ${id ? "" : "required"} placeholder="${id ? "оставить пустым — без смены" : ""}" /></label>
      ${
        id
          ? `<label>Лимит устройств
              <input name="maxDevicesOverride" type="number" min="1" value="${user?.maxDevicesOverride ?? ""}" placeholder="по тарифу: ${planMax}" />
            </label>
            <p class="muted">По тарифу: ${planMax}. Сейчас занято: ${deviceCount}. Эффективный лимит: ${deviceLimit}.</p>`
          : ""
      }
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
    if (id) {
      const overrideRaw = String(fd.get("maxDevicesOverride") ?? "").trim();
      body.maxDevicesOverride = overrideRaw ? Number(overrideRaw) : null;
    }
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
  if (id) bindDeviceActions(id);
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
