import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import test from "node:test";

const source = readFileSync(new URL("../src-tauri/src/login-transition.js", import.meta.url), "utf8");
const signedIn = { accessToken: "test", account: { id: "test" } };
const flush = () => new Promise(resolve => setImmediate(resolve));

function page({ hostname = "chatgpt.com", pathname = "/", session = signedIn, failure = false, pending = false } = {}) {
  class Element {
    isConnected = false;
    inert = false;
    attrs = new Set();
    style = {};
    appendChild(child) { child.isConnected = true; }
    remove() { this.isConnected = false; }
    setAttribute(name) { this.attrs.add(name); }
    removeAttribute(name) { this.attrs.delete(name); }
    attachShadow() { return {}; }
  }
  const root = new Element();
  const body = new Element();
  const timers = new Map();
  const listeners = {};
  let observer;
  let fetchCount = 0;
  const context = {
    location: { hostname, pathname },
    document: { documentElement: root, body, createElement: () => new Element() },
    history: { pushState() {}, replaceState() {} },
    MutationObserver: class { constructor(callback) { observer = callback; } observe() {} },
    AbortController,
    setTimeout(fn) { const id = timers.size + 1; timers.set(id, fn); return id; },
    clearTimeout(id) { timers.delete(id); },
    fetch: async (_url, { signal }) => {
      fetchCount++;
      if (pending) return new Promise((_resolve, reject) => signal.addEventListener("abort", () => reject(new Error("aborted"))));
      if (failure) throw new Error("offline");
      return { ok: true, json: async () => session };
    },
    addEventListener(name, callback) { listeners[name] = callback; }
  };
  context.window = context;
  vm.runInNewContext(source, context);
  return {
    context, body,
    masked: () => root.attrs.has("data-csw-login-transition"),
    fetchCount: () => fetchCount,
    mutate: () => observer(),
    timeout: () => [...timers.values()].forEach(fn => fn()),
    navigate(path) { context.location.pathname = path; context.history.pushState(); }
  };
}

test("covers chat page before session request completes and keeps authenticated page covered", async () => {
  const p = page();
  assert.ok(p.masked());
  assert.ok(p.body.inert);
  await flush();
  assert.ok(p.masked());
  p.mutate();
  assert.equal(p.fetchCount(), 1);
});

test("signed-out page and failed session request restore interactive login controls", async () => {
  for (const options of [{ session: {} }, { failure: true }]) {
    const p = page(options);
    await flush();
    assert.equal(p.masked(), false);
    assert.equal(p.body.inert, false);
  }
});

test("timeout releases a stalled session check", async () => {
  const p = page({ pending: true });
  p.timeout();
  await flush();
  assert.equal(p.masked(), false);
  assert.equal(p.body.inert, false);
});

test("login and challenge routes stay visible, SPA arrival at chat page is masked", async () => {
  const p = page({ pathname: "/auth/login" });
  assert.equal(p.masked(), false);
  assert.equal(p.fetchCount(), 0);
  p.navigate("/");
  assert.ok(p.masked());
  await flush();
  p.navigate("/auth/consent");
  assert.equal(p.masked(), false);
  assert.equal(p.body.inert, false);
});

test("a late signed-out response cannot undo backend confirmation", async () => {
  const p = page({ session: {} });
  p.context.__cswShowLoginTransition();
  await flush();
  assert.ok(p.masked());
});

test("an old chat-page response cannot mask the next interactive route", async () => {
  const p = page();
  p.navigate("/auth/login");
  await flush();
  assert.equal(p.masked(), false);
});

test("OpenAI authorization pages receive no mask or session request", () => {
  const p = page({ hostname: "auth.openai.com", pathname: "/sign-in-with-chatgpt/codex" });
  assert.equal(p.masked(), false);
  assert.equal(p.fetchCount(), 0);
  assert.equal(p.context.__cswShowLoginTransition, undefined);
});
