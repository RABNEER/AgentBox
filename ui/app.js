// AgentBox Mail — Interactive Web Dashboard Client

let currentAccount = null;
let currentMessage = null;
let inboxes = [];
let messages = [];
let eventSource = null;

const presets = {
  github: {
    from: "noreply@github.com",
    subject: "[GitHub] Please verify your device - 849201",
    body: `<div style="font-family: sans-serif; padding: 20px; color: #333;">
      <h2>GitHub Verification Code</h2>
      <p>Please enter the following 6-digit code to sign in to your GitHub account:</p>
      <div style="font-size: 32px; font-weight: bold; letter-spacing: 4px; color: #0969da; margin: 20px 0;">849201</div>
      <p>Or click this direct confirmation link:</p>
      <p><a href="https://github.com/sessions/verified-device?token=gh_sec_918239018239" style="color: #0969da;">https://github.com/sessions/verified-device</a></p>
      <p style="color: #666; font-size: 12px;">This code will expire in 10 minutes.</p>
    </div>`
  },
  aws: {
    from: "no-reply-aws@amazon.com",
    subject: "Amazon Web Services Email Verification (Code: 492018)",
    body: `Hello,\n\nThank you for creating an Amazon Web Services account. Your email verification code is:\n\n492018\n\nEnter this code on the verification page to complete your signup.\n\nAlternatively, verify immediately via:\nhttps://signin.aws.amazon.com/verify?token=aws_auth_837492\n\nRegards,\nAmazon Web Services`
  },
  slack: {
    from: "feedback@slack.com",
    subject: "Sign in to Slack workspace",
    body: `<div style="font-family: sans-serif; padding: 20px;">
      <h2>Sign in with magic link</h2>
      <p>Click the link below to sign in instantly:</p>
      <p><a href="https://app.slack.com/magic-login?code=slk_token_88921" style="background:#4a154b; color:#fff; padding:10px 20px; text-decoration:none; border-radius:4px;">Sign in to Slack</a></p>
      <p>Or enter code manually: <b>930-182</b></p>
    </div>`
  },
  invoice: {
    from: "billing@stripe.com",
    subject: "Invoice #INV-2026-891 from Acme Corp ($49.00)",
    body: `Your invoice for Acme Corp Pro Plan is ready.\nAmount: $49.00 USD\n\nView and pay your invoice online:\nhttps://invoice.stripe.com/i/acct_192837/inv_84920182`
  }
};

// Initialization
document.addEventListener("DOMContentLoaded", async () => {
  setupEventListeners();
  await loadInboxes();
  setupSSE();
});

// Setup Event Listeners
function setupEventListeners() {
  document.getElementById("btn-new-inbox").addEventListener("click", createNewInbox);
  document.getElementById("btn-refresh").addEventListener("click", () => {
    if (currentAccount) loadMessages(currentAccount.id);
  });

  document.getElementById("btn-copy-address").addEventListener("click", () => {
    if (currentAccount) {
      navigator.clipboard.writeText(currentAccount.address);
      showToast("Inbox address copied to clipboard!");
    }
  });

  document.getElementById("btn-copy-otp").addEventListener("click", () => {
    const code = document.getElementById("detected-otp-value").innerText;
    if (code && code !== "------") {
      navigator.clipboard.writeText(code);
      showToast(`Copied OTP: ${code}`);
    }
  });

  // Modal: Inbound Simulate
  const modalSim = document.getElementById("modal-simulate");
  document.getElementById("btn-simulate-modal").addEventListener("click", () => {
    if (!currentAccount) {
      showToast("Please select or create an inbox first!");
      return;
    }
    modalSim.classList.remove("hidden");
    loadPreset("github");
  });
  document.getElementById("btn-close-simulate").addEventListener("click", () => modalSim.classList.add("hidden"));
  document.getElementById("btn-cancel-simulate").addEventListener("click", () => modalSim.classList.add("hidden"));
  document.getElementById("btn-submit-simulate").addEventListener("click", submitSimulatedEmail);

  // Modal Presets
  document.querySelectorAll(".preset-buttons .btn-chip").forEach(btn => {
    btn.addEventListener("click", () => loadPreset(btn.dataset.preset));
  });

  // Modal: Outbound Compose
  const modalComp = document.getElementById("modal-compose");
  document.getElementById("btn-compose-modal").addEventListener("click", () => {
    if (!currentAccount) {
      showToast("Please select or create an inbox first!");
      return;
    }
    modalComp.classList.remove("hidden");
  });
  document.getElementById("btn-close-compose").addEventListener("click", () => modalComp.classList.add("hidden"));
  document.getElementById("btn-cancel-compose").addEventListener("click", () => modalComp.classList.add("hidden"));
  document.getElementById("btn-submit-compose").addEventListener("click", submitOutboundEmail);

  // Search Filter
  document.getElementById("search-input").addEventListener("input", (e) => {
    const q = e.target.value.toLowerCase();
    const filtered = messages.filter(m => 
      m.from_address.toLowerCase().includes(q) ||
      m.subject.toLowerCase().includes(q) ||
      (m.extracted_otp && m.extracted_otp.includes(q))
    );
    renderMessages(filtered);
  });
}

function loadPreset(presetKey) {
  const p = presets[presetKey];
  if (!p) return;
  document.getElementById("sim-from").value = p.from;
  document.getElementById("sim-subject").value = p.subject;
  document.getElementById("sim-body").value = p.body;
}

// Load Inboxes
async function loadInboxes() {
  try {
    const res = await fetch("/v1/accounts");
    if (!res.ok) throw new Error("Failed to load accounts");
    inboxes = await res.json();

    const listEl = document.getElementById("inbox-list");
    listEl.innerHTML = "";

    if (inboxes.length === 0) {
      // Auto-create default inbox
      await createNewInbox("agent-main");
      return;
    }

    inboxes.forEach((acc, idx) => {
      const item = document.createElement("div");
      item.className = `inbox-item ${currentAccount?.id === acc.id || (!currentAccount && idx === 0) ? "active" : ""}`;
      item.innerHTML = `
        <span class="inbox-item-name">${acc.display_name || "Agent Inbox"}</span>
        <span class="inbox-item-address">${acc.address}</span>
      `;
      item.onclick = () => selectAccount(acc);
      listEl.appendChild(item);
    });

    if (!currentAccount && inboxes.length > 0) {
      selectAccount(inboxes[0]);
    }
  } catch (err) {
    console.error(err);
  }
}

// Select Account
async function selectAccount(account) {
  currentAccount = account;
  document.getElementById("current-inbox-address").innerText = account.address;
  
  document.querySelectorAll(".inbox-item").forEach(el => {
    el.classList.toggle("active", el.querySelector(".inbox-item-address").innerText === account.address);
  });

  await loadMessages(account.id);
}

// Create New Inbox
async function createNewInbox(suggestedName = "") {
  const name = typeof suggestedName === "string" && suggestedName.length > 0
    ? suggestedName
    : prompt("Enter a name for the new agent inbox (e.g. qa-tester, bot-worker):", "bot-" + Math.floor(Math.random() * 1000));
  
  if (!name) return;

  try {
    const res = await fetch("/v1/accounts", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ display_name: name })
    });
    if (!res.ok) throw new Error("Failed to create inbox");
    const created = await res.json();
    showToast(`Created inbox: ${created.address}`);
    await loadInboxes();
    selectAccount(created);
  } catch (err) {
    showToast("Error creating inbox: " + err.message);
  }
}

// Load Messages
async function loadMessages(accountId) {
  try {
    const res = await fetch(`/v1/accounts/${accountId}/messages`);
    if (!res.ok) throw new Error("Failed to load messages");
    messages = await res.json();

    document.getElementById("stat-total-messages").innerText = messages.length;
    const otpsCount = messages.filter(m => m.extracted_otp).length;
    document.getElementById("stat-otps-captured").innerText = otpsCount;

    renderMessages(messages);

    // If there is an OTP in the most recent message, show it in the hero banner
    if (messages.length > 0 && messages[0].extracted_otp) {
      showOtpBanner(messages[0]);
    } else {
      document.getElementById("otp-hero-banner").classList.add("hidden");
    }

    if (messages.length > 0) {
      selectMessage(messages[0]);
    } else {
      document.getElementById("detail-pane").innerHTML = `
        <div class="detail-placeholder">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/><path d="M22 6l-10 7L2 6"/></svg>
          <h3>Inbox is empty</h3>
          <p>Click "Simulate Inbound Email" to test instant OTP parsing.</p>
        </div>`;
    }
  } catch (err) {
    console.error(err);
  }
}

// Render Messages List
function renderMessages(items) {
  const container = document.getElementById("messages-list");
  document.getElementById("message-count").innerText = `${items.length} items`;
  container.innerHTML = "";

  if (items.length === 0) {
    container.innerHTML = `
      <div class="empty-state">
        <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect width="20" height="16" x="2" y="4" rx="3"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/></svg>
        <p>No messages found</p>
      </div>`;
    return;
  }

  items.forEach(msg => {
    const card = document.createElement("div");
    card.className = `message-card ${currentMessage?.id === msg.id ? "active" : ""}`;
    const timeStr = new Date(msg.created_at * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

    card.innerHTML = `
      <div class="message-top-row">
        <span class="message-sender">${escapeHtml(msg.from_address)}</span>
        <span class="message-time">${timeStr}</span>
      </div>
      <span class="message-subject">${escapeHtml(msg.subject || "(No Subject)")}</span>
      <div class="message-meta-row">
        ${msg.extracted_otp ? `<span class="badge-otp">OTP: ${escapeHtml(msg.extracted_otp)}</span>` : ""}
      </div>
    `;
    card.onclick = () => selectMessage(msg);
    container.appendChild(card);
  });
}

// Select Message & Render Detail View
function selectMessage(msg) {
  currentMessage = msg;
  document.querySelectorAll(".message-card").forEach(el => el.classList.remove("active"));
  
  if (msg.extracted_otp) {
    showOtpBanner(msg);
  } else {
    document.getElementById("otp-hero-banner").classList.add("hidden");
  }

  const detailPane = document.getElementById("detail-pane");
  const timeFormatted = new Date(msg.created_at * 1000).toLocaleString();
  const links = msg.extracted_links ? JSON.parse(msg.extracted_links) : [];

  detailPane.innerHTML = `
    <div class="detail-header">
      <h2 class="detail-subject">${escapeHtml(msg.subject || "(No Subject)")}</h2>
      <div class="detail-meta-grid">
        <span class="meta-key">From:</span>
        <span class="meta-val">${escapeHtml(msg.from_address)}</span>
        <span class="meta-key">To:</span>
        <span class="meta-val">${escapeHtml(msg.to_address)}</span>
        <span class="meta-key">Date:</span>
        <span class="meta-val">${timeFormatted}</span>
        ${msg.extracted_otp ? `<span class="meta-key">Captured OTP:</span><span class="meta-val" style="color:var(--accent-cyan); font-weight:bold;">${escapeHtml(msg.extracted_otp)}</span>` : ""}
      </div>
    </div>

    <div class="detail-tabs">
      <button class="tab-btn active" onclick="switchDetailTab('html')">Rendered HTML</button>
      <button class="tab-btn" onclick="switchDetailTab('text')">Plain Text</button>
      <button class="tab-btn" onclick="switchDetailTab('links')">Detected Links (${links.length})</button>
    </div>

    <div class="detail-body-container">
      <div id="tab-content-html" class="tab-pane">
        ${msg.body_html ? `<div class="email-html-view">${msg.body_html}</div>` : `<div class="email-text-view">${escapeHtml(msg.body_text || "")}</div>`}
      </div>
      <div id="tab-content-text" class="tab-pane hidden">
        <div class="email-text-view">${escapeHtml(msg.body_text || "(No plain text version available)")}</div>
      </div>
      <div id="tab-content-links" class="tab-pane hidden">
        <div class="email-text-view">
          ${links.length > 0 ? links.map(l => `<p style="margin-bottom: 8px;"><a href="${escapeHtml(l)}" target="_blank" style="color:var(--accent-cyan); word-break:break-all;">🔗 ${escapeHtml(l)}</a></p>`).join("") : "No action links detected in this message."}
        </div>
      </div>
    </div>
  `;
}

window.switchDetailTab = function(tabName) {
  document.querySelectorAll(".tab-btn").forEach(btn => btn.classList.remove("active"));
  document.querySelectorAll(".tab-pane").forEach(pane => pane.classList.add("hidden"));

  const targetPane = document.getElementById(`tab-content-${tabName}`);
  if (targetPane) targetPane.classList.remove("hidden");

  event.target.classList.add("active");
};

function showOtpBanner(msg) {
  const banner = document.getElementById("otp-hero-banner");
  document.getElementById("detected-otp-value").innerText = msg.extracted_otp;

  const linksContainer = document.getElementById("detected-links-container");
  linksContainer.innerHTML = "";
  const links = msg.extracted_links ? JSON.parse(msg.extracted_links) : [];

  if (links.length > 0) {
    const firstLink = links[0];
    linksContainer.innerHTML = `
      <a href="${escapeHtml(firstLink)}" target="_blank" class="action-link-btn">
        <span>Verify Device / Link</span>
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
      </a>
    `;
  }

  banner.classList.remove("hidden");
}

// Submit Inbound Simulation
async function submitSimulatedEmail() {
  if (!currentAccount) return;

  const from = document.getElementById("sim-from").value;
  const subject = document.getElementById("sim-subject").value;
  const body = document.getElementById("sim-body").value;

  try {
    const res = await fetch("/v1/inbound", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        to: currentAccount.address,
        from: from,
        subject: subject,
        body: body
      })
    });
    if (!res.ok) throw new Error("Inbound ingestion failed");
    
    document.getElementById("modal-simulate").classList.add("hidden");
    showToast("⚡ Inbound email received & processed!");
    await loadMessages(currentAccount.id);
  } catch (err) {
    showToast("Error: " + err.message);
  }
}

// Submit Outbound Email
async function submitOutboundEmail() {
  if (!currentAccount) return;

  const to = document.getElementById("out-to").value;
  const subject = document.getElementById("out-subject").value;
  const body = document.getElementById("out-body").value;

  if (!to || !body) {
    showToast("Please enter recipient and body");
    return;
  }

  try {
    const res = await fetch(`/v1/accounts/${currentAccount.id}/messages`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ to: [to], subject: subject, text: body })
    });
    if (!res.ok) throw new Error("Failed to send message");

    document.getElementById("modal-compose").classList.add("hidden");
    showToast("✉️ Outbound email dispatched!");
  } catch (err) {
    showToast("Error: " + err.message);
  }
}

// Server-Sent Events (SSE) for Realtime updates
function setupSSE() {
  if (eventSource) eventSource.close();
  eventSource = new EventSource("/v1/events");

  eventSource.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data);
      if (data.type === "new_message") {
        showToast(`⚡ New email from ${data.message.from_address}!`);
        if (currentAccount && currentAccount.address === data.message.to_address) {
          loadMessages(currentAccount.id);
        }
      }
    } catch (e) {
      console.error("SSE parse error", e);
    }
  };

  eventSource.onerror = () => {
    // Reconnects automatically
  };
}

function showToast(msg) {
  const toast = document.getElementById("toast");
  toast.innerText = msg;
  toast.classList.remove("hidden");
  setTimeout(() => toast.classList.add("hidden"), 3500);
}

function escapeHtml(str) {
  if (!str) return "";
  const div = document.createElement("div");
  div.innerText = str;
  return div.innerHTML;
}

// =========================================================================
// 1-Click Connect AI Agents Wizard Functions
// =========================================================================

let detectedIntegrations = [];
let availableIdentities = [];
let isCreatingNewAgent = false;

async function openConnectAgentsModal() {
  const modal = document.getElementById("connect-agents-modal");
  if (!modal) return;
  modal.classList.add("open");
  await detectIntegrations();
  if (window.lucide) window.lucide.createIcons();
}

function closeConnectAgentsModal() {
  const modal = document.getElementById("connect-agents-modal");
  if (modal) modal.classList.remove("open");
}

async function detectIntegrations() {
  const listEl = document.getElementById("frameworks-list");
  const badgeEl = document.getElementById("detected-count-badge");
  const selectEl = document.getElementById("agent-identity-select");
  if (!listEl) return;

  listEl.innerHTML = `<div class="p-4 text-center text-xs text-base-500 border border-base-800 rounded-xl bg-base-950 col-span-2">Scanning your computer for AI frameworks...</div>`;

  try {
    const res = await fetch("/v1/integrations/detect");
    if (!res.ok) throw new Error("Failed to detect frameworks");
    const data = await res.json();
    detectedIntegrations = data.frameworks || [];
    availableIdentities = data.identities || [];

    const detectedCount = detectedIntegrations.filter(f => f.detected).length;
    if (badgeEl) {
      badgeEl.textContent = `${detectedCount} of ${detectedIntegrations.length} Detected`;
    }

    // Render Framework Cards
    listEl.innerHTML = detectedIntegrations.map(fw => `
      <label class="setup-card p-3.5 flex items-start gap-3 cursor-pointer rounded-xl border border-base-700 bg-base-850 hover:bg-base-800 transition-all ${fw.detected ? 'border-emerald-500/40 bg-emerald-500/[0.03]' : 'opacity-85'}">
        <input type="checkbox" value="${fw.id}" ${fw.detected ? 'checked' : ''} class="email-check mt-0.5" id="fw-${fw.id}">
        <div class="flex-1 min-w-0">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold text-white tracking-tight">${escapeHtml(fw.name)}</span>
            <span class="text-[9px] font-mono px-2 py-0.5 rounded-full ${fw.detected ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-semibold' : 'bg-base-700 text-base-400'}">${escapeHtml(fw.status)}</span>
          </div>
          <p class="text-[11px] text-base-400 mt-0.5 truncate">${escapeHtml(fw.description)}</p>
          <span class="text-[10px] text-base-500 font-mono block mt-1 truncate">${escapeHtml(fw.config_path)}</span>
        </div>
      </label>
    `).join("");

    // Populate Identities Dropdown
    if (selectEl) {
      if (availableIdentities.length > 0) {
        selectEl.innerHTML = availableIdentities.map(id => `
          <option value="${id.id}">${escapeHtml(id.name)} (${escapeHtml(id.email_address)})</option>
        `).join("") + `<option value="new">+ Create New Identity...</option>`;
      } else {
        selectEl.innerHTML = `
          <option value="default">General Agent (agent@${window.location.hostname || 'local.agentbox'})</option>
          <option value="new">+ Create New Custom Identity...</option>
        `;
      }
    }
  } catch (err) {
    listEl.innerHTML = `<div class="p-4 text-center text-xs text-red-400 border border-red-900/40 rounded-xl bg-red-950/20 col-span-2">Scan error: ${escapeHtml(err.message)}</div>`;
  }
}

function toggleNewAgentDrawer() {
  const form = document.getElementById("new-identity-form");
  const existing = document.getElementById("existing-identity-section");
  const btn = document.getElementById("toggle-new-agent-btn");
  if (!form) return;

  isCreatingNewAgent = !isCreatingNewAgent;
  if (isCreatingNewAgent) {
    form.classList.remove("hidden");
    existing.classList.add("hidden");
    btn.innerHTML = `<i data-lucide="x" class="w-3 h-3"></i> Use Existing Identity`;
  } else {
    form.classList.add("hidden");
    existing.classList.remove("hidden");
    btn.innerHTML = `<i data-lucide="plus" class="w-3 h-3"></i> Create New Identity`;
  }
  if (window.lucide) window.lucide.createIcons();
}

async function submitConnectAgents() {
  const submitBtn = document.getElementById("connect-submit-btn");
  const resultsSection = document.getElementById("connect-results-section");
  const resultsList = document.getElementById("connect-results-list");

  const selectedFrameworks = Array.from(document.querySelectorAll("#frameworks-list input[type=checkbox]:checked"))
    .map(cb => cb.value);

  if (selectedFrameworks.length === 0) {
    showToast("Please select at least one AI framework to connect.");
    return;
  }

  let payload = {
    frameworks: selectedFrameworks
  };

  if (isCreatingNewAgent) {
    const name = document.getElementById("new-agent-name").value.trim();
    if (!name) {
      showToast("Please enter an Agent Name");
      return;
    }
    const email = document.getElementById("new-agent-email").value.trim() || undefined;
    const caps = Array.from(document.querySelectorAll("#new-agent-caps input[type=checkbox]:checked"))
      .map(cb => cb.value);

    payload.create_agent = {
      name,
      email,
      capabilities: caps
    };
  } else {
    const selectEl = document.getElementById("agent-identity-select");
    const val = selectEl ? selectEl.value : null;
    if (val === "new") {
      toggleNewAgentDrawer();
      return;
    }
    payload.agent_id = val && val !== "default" ? val : undefined;
  }

  submitBtn.disabled = true;
  submitBtn.innerHTML = `<i data-lucide="loader" class="w-4 h-4 animate-spin"></i> Connecting...`;
  if (window.lucide) window.lucide.createIcons();

  resultsSection.classList.remove("hidden");
  resultsList.innerHTML = `<div class="text-base-400 text-xs animate-pulse">Injecting MCP configuration into selected runtimes...</div>`;

  try {
    const res = await fetch("/v1/integrations/connect", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });

    if (!res.ok) throw new Error("Failed to connect agents");
    const data = await res.json();

    resultsList.innerHTML = data.results.map(r => `
      <div class="flex items-center justify-between p-2.5 rounded-lg bg-base-900 border border-base-800">
        <div class="flex items-center gap-2">
          <span class="w-5 h-5 rounded-full bg-emerald-500/20 text-emerald-400 flex items-center justify-center text-xs">✓</span>
          <div>
            <span class="font-bold text-white text-xs">${escapeHtml(r.name)}</span>
            <span class="text-base-500 text-[10px] block font-mono">${escapeHtml(r.path)}</span>
          </div>
        </div>
        <span class="text-emerald-400 text-xs font-semibold">${escapeHtml(r.status)}</span>
      </div>
    `).join("") + `
      <div class="mt-4 p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-center space-y-1">
        <h4 class="text-sm font-bold text-emerald-400">🎉 Your AI agents now have email.</h4>
        <p class="text-xs text-base-300 font-mono">Assigned Identity: <span class="text-white font-semibold">${escapeHtml(data.agent_email)}</span></p>
      </div>
    `;

    submitBtn.innerHTML = `<i data-lucide="check" class="w-4 h-4"></i> Connected!`;
    showToast(`🎉 Connected ${data.results.length} AI Agents to ${data.agent_email}!`);
  } catch (err) {
    resultsList.innerHTML = `<div class="text-red-400 text-xs p-2">Error connecting: ${escapeHtml(err.message)}</div>`;
    submitBtn.innerHTML = `<i data-lucide="zap" class="w-4 h-4"></i> Retry Connection`;
    submitBtn.disabled = false;
  }
}
