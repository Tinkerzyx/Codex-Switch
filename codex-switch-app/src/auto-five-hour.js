/* Scheduling uses only fresh server usage, never cached reset display strings. */
function createAutoFiveHourScheduler(options) {
  var windowSeconds = 18000;
  var observations = new Map();
  var queue = [];
  var draining = false;
  var enabled = false;
  var states = options.load() || {};
  if (typeof states !== 'object' || Array.isArray(states)) states = {};

  function accountKey(profile) {
    return profile && profile.accountId && profile.accountId !== '-' ? profile.accountId : null;
  }
  function stateFor(key) {
    if (!states[key] || typeof states[key] !== 'object') states[key] = {};
    return states[key];
  }
  function save() { options.save(states); }
  function eligible(key, now) {
    var state = stateFor(key);
    return !(state.confirmedUntil > now) && !(state.retryAt > now);
  }
  function rolling(sample, now) {
    return sample && now - sample.at <= 180 && now >= sample.at &&
      sample.left >= 99.999 && sample.reset > now && Math.abs(sample.reset - sample.at - windowSeconds) <= 90;
  }
  function log(stage, profile, details) {
    if (options.log) options.log(stage, profile, details);
  }
  function observe(profile, usage) {
    if (!enabled || !accountKey(profile)) return;
    var now = options.now();
    var key = accountKey(profile);
    var reset = usage.primaryResetAt;
    var left = usage.primaryLeft;
    if (typeof reset !== 'number' || !Number.isFinite(reset) || typeof left !== 'number' || !Number.isFinite(left)) {
      observations.delete(key);
      return;
    }
    var sample = { at: now, reset: reset, left: left };
    if (!rolling(sample, now)) {
      observations.delete(key);
      queue = queue.filter(function (item) { return item.key !== key; });
      return;
    }
    var previous = observations.get(key);
    // Duplicate/manual refreshes must not replace the minute-spaced baseline.
    if (previous && now - previous.at < 30 && now >= previous.at) return;
    observations.set(key, sample);
    if (!eligible(key, now) || !previous) return;
    var elapsed = now - previous.at;
    var movement = reset - previous.reset;
    if (elapsed < 30 || elapsed > 180 || movement < 15 || Math.abs(movement - elapsed) > 15) return;
    if (queue.some(function (item) { return item.key === key; })) return;
    queue.push({ key: key, name: profile.name, queuedAt: now });
    log('auto_queued', profile, { resetAt: reset, sampleSeconds: elapsed, movementSeconds: movement, queueLength: queue.length });
  }
  function recordResult(profile, result, error) {
    var key = accountKey(profile);
    if (!key) return;
    var now = options.now();
    var state = stateFor(key);
    observations.delete(key);
    queue = queue.filter(function (item) { return item.key !== key; });
    if (!error && result && result.confirmed && result.usage && result.usage.primaryResetAt > now) {
      state.confirmedUntil = result.usage.primaryResetAt;
      state.attempts = 0;
      state.retryAt = 0;
      log('auto_confirmed', profile, { resetAt: state.confirmedUntil });
    } else {
      // The next retry still needs two fresh rolling observations.
      var delays = [60, 300, 900, 3600];
      var attempt = Math.max(1, Number(state.attempts) || 1);
      state.retryAt = now + delays[Math.min(attempt - 1, delays.length - 1)];
      log('auto_retry_scheduled', profile, { retryAt: state.retryAt, attempt: attempt, unconfirmed: !error });
    }
    save();
  }
  function markStarted(profile) {
    var key = accountKey(profile);
    if (!key) return;
    var state = stateFor(key);
    state.attempts = Math.min((Number(state.attempts) || 0) + 1, 100);
    // Persist a lease before invoking CLI, so app restarts cannot immediately duplicate a run.
    state.retryAt = options.now() + 600;
    save();
  }
  async function drain() {
    if (!enabled || draining || !options.canRun()) return;
    draining = true;
    try {
      while (enabled && queue.length && options.canRun()) {
        var item = queue.shift();
        var profile = options.profiles().find(function (p) { return p.name === item.name && accountKey(p) === item.key; });
        var now = options.now();
        if (!profile || !eligible(item.key, now) || !rolling(observations.get(item.key), now)) {
          log('auto_queue_skipped', profile || { name: item.name }, { reason: 'profile_changed_or_stale_or_cooldown' });
          continue;
        }
        markStarted(profile);
        try {
          var result = await options.run(profile);
          recordResult(profile, result, null);
        } catch (error) {
          recordResult(profile, null, error);
        }
      }
    } finally { draining = false; }
  }
  function setEnabled(value) {
    enabled = !!value;
    observations.clear();
    queue = [];
  }
  return { observe: observe, drain: drain, setEnabled: setEnabled, markStarted: markStarted, recordResult: recordResult };
}
