// The online transport: WebRTC data channels between up to eight browsers,
// one reliable ordered channel per pair, JSON text from the core's online
// module (crates/core/src/online.rs) in and out. It decides nothing about the
// match: it connects, carries text, and says who arrived and who left. Ported
// from Chromatic, which has it from Crabble, Honey Hole, Ape Race and Mandible.
//
// kits/online exempts this file, in crates/core/tests/boundary.rs, from the
// page scan's ban on network APIs: it may name RTCPeerConnection, and
// WebSocket for the relay probe alone (`probeRelay`, which opens only pinned
// relays; the CSP's connect-src allows nothing else). Numbers other than 0
// and 1, and Math and the rest, stay banned; every timing and id is in
// web/net/config.js or comes from the core.
//
// A full mesh is what Trystero hands back, and the core uses only the star
// through the host (crates/core/src/online.rs): a joiner's links to the other
// joiners stay idle.
//
// Two introductions reach the same links:
//
//   * a room code: Trystero (Nostr strategy, vendored in web/net/vendor/ and
//     imported only here, only when hostRoom or joinRoom is called) meets
//     the other browsers on public relays and hands back one connection per
//     peer. A Trystero room is already a mesh.
//   * a pasted code: the host's offer code and the joiner's answer code carry
//     the whole handshake (no trickle), compressed. No relay is involved. The
//     mesh is completed through the host: a newcomer is told who is already
//     there, and the offers and answers between joiners travel over the
//     data channels they already have to the host.
//
// From the channel down the paths are the same. Each side creates one
// *negotiated* channel on a fixed stream id, so there is no ondatachannel
// event to miss (floodline quad_rtc.js). A message sent before the other end
// has created its side of a negotiated channel is lost, so every link repeats
// a hello until the other side acknowledges it, and no data is sent on a link
// until the other side's hello has arrived (vagrancy rtc.js). Data that beats
// a hello is kept and delivered after it.
//
// A peer is the other browser, keyed by its stable id (Trystero's selfId, or
// a random id sent in the hello on the pasted-code path), whatever
// connections come and go under it: Trystero may replace a connection to the
// same browser, and the new link is swapped in under the same peer while
// unsent text waits in the peer's queue (vagrancy, "the first version treated
// the new connection as a stranger").
//
// Before a room opens, the core (crates/core/src/online/relays.rs) has the
// page probe the pinned relays and opens the room on the live ones only; it
// re-probes and reopens on a timeout, and gives up with an error key rather
// than hang.
//
// Events go to onEvent as {kind, peer, text, host}:
//   connected     peer arrived; `host` says whether it called itself the host
//   message       text from peer
//   disconnected  peer left (pasted code: its link closed; room: Trystero said so)
//   error         text is a key for data/: online.no_bundle, online.no_relay,
//                 online.no_path, online.signal_failed
//
// Every frame on a channel starts with one character saying what it is.

import { CONFIG, iceServers } from "./config.js";

const HELLO = "h";
const ACK = "k";
const DATA = "d";
// Pasted-code mesh only: an offer or answer relayed through the host.
const SIGNAL = "s";
// Pasted-code mesh only: the host tells a newcomer whom to connect to.
const MEET = "p";

const OFFER = "O";
const ANSWER = "A";

// The one session this page has, or null.
let S = null;

function rtcConfig() {
  return { iceServers: iceServers() };
}

function open(mode, isHost, onEvent, selfId) {
  leave();
  S = {
    mode,
    isHost,
    onEvent,
    selfId,
    closed: false,
    room: null,
    peers: new Map(),
    links: new Set(),
    pending: null,
    offers: new Map(),
    timers: new Set(),
  };
  return S;
}

function emit(s, ev) {
  if (!s.closed) s.onEvent(ev);
}

function trySend(ch, text) {
  if (ch.readyState !== "open") return false;
  try {
    ch.send(text);
    return true;
  } catch (e) {
    return false;
  }
}

// ---- links -----------------------------------------------------------------

function attach(s, pc, mine) {
  const ch = pc.createDataChannel("game", {
    negotiated: true,
    id: CONFIG.channelId,
    ordered: true,
  });
  // `key`: the peer this link must belong to, when known before its hello
  // (Trystero's id for it, or the joiner the host introduced).
  const link = { pc, ch, mine, key: null, peer: null, ready: false, acked: false, gone: false, timer: null, early: [] };
  s.links.add(link);
  const hello = () => {
    if (link.gone || link.acked) return;
    trySend(ch, HELLO + JSON.stringify({ id: s.selfId, host: s.isHost }));
  };
  ch.onopen = () => {
    hello();
    link.timer = setInterval(hello, CONFIG.helloRepeatMs);
  };
  ch.onmessage = (e) => frame(s, link, String(e.data));
  ch.onclose = () => lost(s, link);
  pc.addEventListener("connectionstatechange", () => {
    if (pc.connectionState === "failed" || pc.connectionState === "closed") lost(s, link);
  });
  return link;
}

function frame(s, link, text) {
  if (s.closed || link.gone) return;
  const kind = text.charAt(0);
  const body = text.slice(1);
  if (kind === ACK) {
    link.acked = true;
    clearInterval(link.timer);
    return;
  }
  if (kind === HELLO) {
    trySend(link.ch, ACK);
    if (!link.ready) greet(s, link, JSON.parse(body));
    return;
  }
  if (!link.ready) {
    // The sender has heard our hello, so its end exists; its own hello is
    // still a repeat away. Keep this until then.
    link.early.push(text);
    return;
  }
  if (kind === DATA) {
    emit(s, { kind: "message", peer: link.peer.id, text: body });
  } else if (kind === SIGNAL) {
    signal(s, link.peer, JSON.parse(body));
  } else if (kind === MEET && link.peer.host) {
    meet(s, link.peer, JSON.parse(body));
  }
}

function greet(s, link, hello) {
  const id = link.key === null ? String(hello.id) : link.key;
  if (id === s.selfId || (link.key !== null && String(hello.id) !== link.key && s.mode === "code")) {
    hangUp(s, link);
    return;
  }
  link.ready = true;
  let peer = s.peers.get(id);
  const fresh = !peer;
  if (fresh) {
    peer = { id, link: null, queue: [], host: hello.host === true };
    s.peers.set(id, peer);
  }
  // A replaced connection to the same browser: the new link takes over.
  if (peer.link && peer.link !== link) hangUp(s, peer.link);
  peer.link = link;
  link.peer = peer;
  if (fresh) {
    emit(s, { kind: "connected", peer: id, host: peer.host });
    if (s.mode === "code" && s.isHost) introduce(s, peer);
  }
  flush(s, peer);
  for (const text of link.early.splice(0)) frame(s, link, text);
}

function flush(s, peer) {
  while (peer.queue.length && peer.link && peer.link.ready && !peer.link.gone) {
    if (!trySend(peer.link.ch, peer.queue[0])) {
      // A channel can close without a close event (its connection closed
      // under it): let it go like any lost link, so the text waits for the
      // next one instead of queueing behind a dead one for ever.
      if (peer.link.ch.readyState === "closed") lost(s, peer.link);
      return;
    }
    peer.queue.shift();
  }
}

function sendFrame(s, peer, text) {
  peer.queue.push(text);
  flush(s, peer);
}

function lost(s, link) {
  if (link.gone) return;
  link.gone = true;
  clearInterval(link.timer);
  s.links.delete(link);
  const peer = link.peer;
  if (!peer || peer.link !== link) return;
  peer.link = null;
  // By pasted code there is nobody else to say a peer left: a closed link is
  // the leaving. By room code Trystero says so (onPeerLeave), and a closed
  // link may be a connection being replaced.
  if (s.mode === "code") depart(s, peer);
}

function depart(s, peer) {
  if (s.peers.get(peer.id) !== peer) return;
  s.peers.delete(peer.id);
  if (peer.link) hangUp(s, peer.link);
  emit(s, { kind: "disconnected", peer: peer.id });
}

// Our channel is always ours to close; the connection under it only when we
// made it. Closing one of Trystero's makes the other end log an abort.
function hangUp(s, link) {
  link.gone = true;
  clearInterval(link.timer);
  s.links.delete(link);
  try {
    link.ch.close();
    if (link.mine) link.pc.close();
  } catch (e) {
    // already closing
  }
}

// ---- which relays are up ---------------------------------------------------

// `?relay=none`: no relay is contacted. A probe reports every pinned relay
// live and the room then waits with no one in it. For checks; a player never
// needs it.
function noRelay() {
  return new URLSearchParams(location.search).get("relay") === "none";
}

// One pinned relay: open it, send the core's REQ, and call it live if EOSE
// comes back before the timeout. Anything else (a refused or redirected
// handshake, CLOSED or NOTICE, silence) is dead.
function probeRelay(url, timeoutMs, req) {
  return new Promise((resolve) => {
    let ws = null;
    let settled = false;
    let timer = null;
    const done = (live) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        if (ws) ws.close();
      } catch (e) {
        // already closing
      }
      resolve(live);
    };
    timer = setTimeout(() => done(false), timeoutMs);
    try {
      ws = new WebSocket(url);
    } catch (e) {
      done(false);
      return;
    }
    ws.onopen = () => ws.send(req);
    ws.onmessage = (e) => {
      let msg = null;
      try {
        msg = JSON.parse(String(e.data));
      } catch (err) {
        return;
      }
      if (!Array.isArray(msg)) return;
      if (msg[0] === "EOSE") done(true);
      else if (msg[0] === "CLOSED" || msg[0] === "NOTICE") done(false);
    };
    ws.onerror = () => done(false);
    ws.onclose = () => done(false);
  });
}

/**
 * Probe every pinned relay at once with the core's REQ text. Resolves to the
 * live ones, in pinned order.
 */
export async function probeRelays(timeoutMs, req) {
  if (noRelay()) return [...CONFIG.relays];
  const live = await Promise.all(CONFIG.relays.map((url) => probeRelay(url, timeoutMs, req)));
  return CONFIG.relays.filter((url, i) => live[i]);
}

// ---- a room code, through Trystero -----------------------------------------

// `relays`: the core's live set (default: every pinned relay); a relay not
// pinned is dropped whatever the caller says. Every one left is used:
// Trystero takes `relayConfig.urls` whole (its hashed subset,
// `relayConfig.redundancy`, applies only to its own default list). Trystero
// fixes its relays when the first room of a page opens and keeps them until
// the last room leaves; `open` leaves the previous room first, so a reopened
// room gets the new set.
async function room(isHost, roomCode, onEvent, relays) {
  const s = open("room", isHost, onEvent, null);
  const urls = (relays || CONFIG.relays).filter((url) => CONFIG.relays.includes(url));
  if (noRelay()) return null;
  if (!urls.length) {
    emit(s, { kind: "error", text: "online.no_relay" });
    return null;
  }
  let lib;
  try {
    lib = await import(CONFIG.bundle);
  } catch (e) {
    emit(s, { kind: "error", text: "online.no_bundle" });
    return null;
  }
  if (s.closed) return null;
  s.selfId = lib.selfId;
  const r = lib.joinRoom(
    {
      appId: CONFIG.appId,
      password: CONFIG.password,
      relayConfig: { urls },
      rtcConfig: rtcConfig(),
    },
    roomCode,
    {
      onJoinError: (d) => {
        // A failed attempt to reach a browser we already have a working link
        // to is Trystero retrying underneath it, not the end of it.
        const known = d && d.peerId ? s.peers.get(d.peerId) : null;
        if (known && known.link) return;
        emit(s, { kind: "error", text: d && d.peerId ? "online.no_path" : "online.no_relay" });
      },
    },
  );
  s.room = r;
  // Trystero 0.25.4 takes these as properties; assigning onPeerJoin also
  // replays the peers already there.
  r.onPeerJoin = (pid) => {
    const pc = r.getPeers()[pid];
    if (!pc) return;
    for (const link of s.links) if (link.pc === pc) return;
    const link = attach(s, pc, false);
    link.key = pid;
  };
  r.onPeerLeave = (pid) => {
    const peer = s.peers.get(pid);
    if (peer) depart(s, peer);
    for (const link of [...s.links]) if (link.key === pid) hangUp(s, link);
  };
  const timer = setTimeout(() => {
    if (s.closed || s.peers.size) return;
    const sockets = Object.values(lib.getRelaySockets ? lib.getRelaySockets() : {});
    if (!sockets.some((w) => w && w.readyState === w.OPEN)) {
      emit(s, { kind: "error", text: "online.no_relay" });
    }
  }, CONFIG.relayTimeoutMs);
  s.timers.add(timer);
  return s.selfId;
}

/**
 * Host a room by code on `relays` (default: every pinned relay), leaving any
 * room already open. Resolves to this browser's id, or null.
 */
export function hostRoom(roomCode, onEvent, relays) {
  return room(true, roomCode, onEvent, relays);
}

/** Join a room by code; as hostRoom. */
export function joinRoom(roomCode, onEvent, relays) {
  return room(false, roomCode, onEvent, relays);
}

// ---- a pasted code ---------------------------------------------------------

function gathered(s, pc) {
  if (pc.iceGatheringState === "complete") return Promise.resolve();
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, CONFIG.gatherTimeoutMs);
    s.timers.add(timer);
    pc.addEventListener("icegatheringstatechange", () => {
      if (pc.iceGatheringState === "complete") resolve();
    });
  });
}

async function offerFrom(s, pc) {
  await pc.setLocalDescription(await pc.createOffer());
  await gathered(s, pc);
  return pc.localDescription.sdp;
}

async function answerFrom(s, pc, offerSdp) {
  await pc.setRemoteDescription({ type: "offer", sdp: offerSdp });
  await pc.setLocalDescription(await pc.createAnswer());
  await gathered(s, pc);
  return pc.localDescription.sdp;
}

/** Start hosting by pasted code. Resolves to the first offer code. */
export function hostByCode(onEvent) {
  open("code", true, onEvent, crypto.randomUUID());
  return inviteByCode();
}

/** Host: a new offer code for the next joiner. */
export async function inviteByCode() {
  const s = S;
  if (!s || s.mode !== "code" || !s.isHost) throw new Error("online.not_hosting");
  if (s.pending && !s.pending.ready) hangUp(s, s.pending);
  const pc = new RTCPeerConnection(rtcConfig());
  // Before createOffer, so the offer has an m=application section.
  const link = attach(s, pc, true);
  s.pending = link;
  return pack(OFFER, await offerFrom(s, pc));
}

/** Host: the joiner's answer code for the current offer. */
export async function acceptAnswer(answerCode) {
  const s = S;
  const link = s ? s.pending : null;
  if (!link) throw new Error("online.no_offer");
  const sdp = await unpack(ANSWER, answerCode);
  await link.pc.setRemoteDescription({ type: "answer", sdp });
  if (s.pending === link) s.pending = null;
}

/** Join by the host's offer code. Resolves to the answer code to send back. */
export async function joinByCode(offerCode, onEvent) {
  const s = open("code", false, onEvent, crypto.randomUUID());
  const sdp = await unpack(OFFER, offerCode);
  const pc = new RTCPeerConnection(rtcConfig());
  attach(s, pc, true);
  return pack(ANSWER, await answerFrom(s, pc, sdp));
}

// Host: tell a newcomer who is already here. It offers to each of them
// through us. Peers become ready one at a time, so of any two joiners exactly
// one is told about the other.
function introduce(s, peer) {
  const others = [...s.peers.keys()].filter((id) => id !== peer.id);
  if (others.length) sendFrame(s, peer, MEET + JSON.stringify(others));
}

// Newcomer: offer to each peer the host named.
function meet(s, host, ids) {
  for (const id of ids) {
    if (s.peers.has(id) || s.offers.has(id)) continue;
    const link = attach(s, new RTCPeerConnection(rtcConfig()), true);
    link.key = id;
    s.offers.set(id, link);
    offerFrom(s, link.pc).then(
      (sdp) => sendFrame(s, host, SIGNAL + JSON.stringify({ to: id, kind: OFFER, sdp })),
      () => emit(s, { kind: "error", text: "online.signal_failed" }),
    );
  }
}

function signal(s, from, msg) {
  if (s.isHost) {
    // Forward between two joiners, naming who it came from.
    const target = s.peers.get(String(msg.to));
    if (target) sendFrame(s, target, SIGNAL + JSON.stringify({ from: from.id, kind: msg.kind, sdp: msg.sdp }));
    return;
  }
  if (!from.host) return;
  const id = String(msg.from);
  if (msg.kind === OFFER) {
    if (s.peers.has(id)) return;
    const link = attach(s, new RTCPeerConnection(rtcConfig()), true);
    link.key = id;
    answerFrom(s, link.pc, msg.sdp).then(
      (sdp) => sendFrame(s, from, SIGNAL + JSON.stringify({ to: id, kind: ANSWER, sdp })),
      () => emit(s, { kind: "error", text: "online.signal_failed" }),
    );
  } else if (msg.kind === ANSWER) {
    const link = s.offers.get(id);
    if (!link) return;
    s.offers.delete(id);
    link.pc.setRemoteDescription({ type: "answer", sdp: msg.sdp }).catch(() => {
      emit(s, { kind: "error", text: "online.signal_failed" });
    });
  }
}

// ---- the code text ---------------------------------------------------------
//
// One letter (O for an offer, A for an answer) outside the compressed part,
// so a code pasted into the wrong box is refused by name, then the session
// description, deflated and in base64url.

function b64url(bytes) {
  return btoa(String.fromCharCode(...bytes)).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

function unb64url(text) {
  const s = atob(text.replaceAll("-", "+").replaceAll("_", "/"));
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}

async function through(stream, bytes) {
  const w = stream.writable.getWriter();
  // a garbled code rejects these too, and the read below is what reports it
  w.write(bytes).catch(() => {});
  w.close().catch(() => {});
  return new Uint8Array(await new Response(stream.readable).arrayBuffer());
}

async function pack(kind, sdp) {
  const raw = new TextEncoder().encode(sdp);
  return kind + b64url(await through(new CompressionStream("deflate-raw"), raw));
}

async function unpack(kind, code) {
  const text = String(code || "").replace(/\s+/g, "");
  if (text.charAt(0) !== kind) throw new Error("online.wrong_code");
  try {
    const raw = await through(new DecompressionStream("deflate-raw"), unb64url(text.slice(1)));
    return new TextDecoder().decode(raw);
  } catch (e) {
    throw new Error("online.bad_paste");
  }
}

// ---- sending and leaving ---------------------------------------------------

/** How often the door pumps the core with no animation frames to do it. */
export const pumpMs = CONFIG.pumpMs;

/** This browser's id in the current session, or null. */
export function myId() {
  return S ? S.selfId : null;
}

/** Send text to one peer. Waits in a queue while its link is being replaced. */
export function send(peerId, text) {
  const s = S;
  const peer = s ? s.peers.get(peerId) : null;
  if (!peer) return false;
  sendFrame(s, peer, DATA + text);
  return true;
}

/** Send text to every connected peer. */
export function broadcast(text) {
  const s = S;
  if (!s) return;
  for (const peer of s.peers.values()) sendFrame(s, peer, DATA + text);
}

/** Close every link and leave the room. No events follow. */
export function leave() {
  const s = S;
  if (!s) return;
  S = null;
  s.closed = true;
  for (const timer of s.timers) clearTimeout(timer);
  for (const link of [...s.links]) hangUp(s, link);
  if (s.room) {
    try {
      s.room.leave();
    } catch (e) {
      // already gone
    }
  }
}

// A tab that is going away says so the only way it can: closing its
// connections puts an SCTP shutdown on the wire, so the other ends hear
// within milliseconds instead of waiting out ICE consent freshness (floodline
// measured sixteen seconds). A crash still takes the slow path.
window.addEventListener("pagehide", () => leave());
