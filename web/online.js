// Online play's page side (after Chromatic's web/net.js): carries text between
// the core's room (crates/core/src/online/room.rs) and the transport
// (web/net/rtc.js), and does what the core asks of the transport. It decides
// nothing: the core says what to send, to whom, and when to host, join or
// leave. This is the one page file that may name the transport.
//
// rtc.js, and the vendored bundle behind it, load only when the core first
// asks for a room (a player pressed Host or Join), so loading and idling stay
// on this origin.

let game = null;
let readKeys = null;
let transport = null;
let clock = null;

// A worker's tick, started with online play, for a player whose tab goes to
// the background: such a tab gets no animation frames and its own timers slow
// down, but a worker's timer keeps running.
function startClock() {
  if (clock) return;
  try {
    clock = new Worker("./net-clock.js");
    clock.onmessage = pump;
    clock.postMessage(transport.pumpMs);
  } catch (e) {
    clock = setInterval(pump, transport.pumpMs);
  }
}

function onNet(e) {
  game.net_event(e.kind, e.peer || "", e.text || "", !!e.host, performance.now());
}

function onCode(text) {
  game.net_event("code", "", text, false, performance.now());
}

async function request(req) {
  if (req.op === "leave") {
    if (transport) transport.leave();
    return;
  }
  try {
    if (!transport) transport = await import("./net/rtc.js");
    startClock();
    if (req.op === "probe_relays") {
      const live = await transport.probeRelays(req.timeout_ms, req.req);
      game.net_event("relays", "", JSON.stringify({ id: req.id, live }), false, performance.now());
    } else if (req.op === "host_room") await transport.hostRoom(req.room, onNet, req.relays);
    else if (req.op === "join_room") await transport.joinRoom(req.room, onNet, req.relays);
    else if (req.op === "host_by_code") onCode(await transport.hostByCode(onNet));
    else if (req.op === "invite_by_code") onCode(await transport.inviteByCode());
    else if (req.op === "join_by_code") onCode(await transport.joinByCode(req.invite, onNet));
    else if (req.op === "accept_reply") await transport.acceptAnswer(req.reply);
  } catch (err) {
    game.net_event("error", "", (err && err.message) || "online.signal_failed", false, performance.now());
  }
}

/** One turn of the core: its room's work, then what it asked for. */
export function pump() {
  const result = JSON.parse(game.net_pump(performance.now(), readKeys()));
  if (!result.ok) return;
  const p = result.ok;
  // the text first: a parting word goes out before the links close
  if (transport) p.out.forEach((m) => transport.send(m.to, m.text));
  p.requests.forEach(request);
}

/** `keys` is read once a turn: the input bits now held. */
export function startNet(theGame, keys) {
  game = theGame;
  readKeys = keys;
}
