#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

function usage() {
  console.error('Usage: node scripts/agent-trace-report.js <trace.jsonl> [--out <report.md>]');
}

function parseArgs(argv) {
  if (argv.length < 3) {
    usage();
    process.exit(1);
  }
  const args = argv.slice(2);
  const tracePath = path.resolve(args[0]);
  let outPath = null;
  for (let i = 1; i < args.length; i += 1) {
    if (args[i] === '--out') {
      outPath = path.resolve(args[i + 1]);
      i += 1;
    }
  }
  return { tracePath, outPath };
}

function parseLines(text) {
  const events = [];
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (!line) continue;
    try {
      events.push(JSON.parse(line));
    } catch (err) {
      // skip malformed lines to keep report generation resilient
    }
  }
  return events;
}

function toMs(ts) {
  const n = Date.parse(ts || '');
  return Number.isFinite(n) ? n : null;
}

function ensureScreen(map, id, title) {
  if (!map.has(id)) {
    map.set(id, {
      id,
      title,
      views: 0,
      actions: 0,
      validationErrors: 0,
      systemErrors: 0,
      dwellMs: 0,
      backtracks: 0,
    });
  }
  return map.get(id);
}

function analyze(events) {
  const screens = new Map();
  const loops = new Map();
  let current = null;
  let currentTs = null;
  let prevScreenId = null;
  let sessionEndTs = null;

  for (const evt of events) {
    const payload = evt.payload || {};
    const tsMs = toMs(evt.ts);

    if (evt.event === 'screen_view') {
      const id = payload.screen_id || 'unknown';
      const title = payload.title || id;
      const screen = ensureScreen(screens, id, title);
      screen.views += 1;

      if (current && tsMs !== null && currentTs !== null && tsMs > currentTs) {
        current.dwellMs += tsMs - currentTs;
      }

      if (prevScreenId && prevScreenId !== id && current && current.id === prevScreenId) {
        const priorViews = screen.views;
        if (priorViews > 1) {
          screen.backtracks += 1;
        }
      }

      current = screen;
      currentTs = tsMs;
      prevScreenId = id;
      continue;
    }

    if (evt.event === 'user_action') {
      if (current) current.actions += 1;
      continue;
    }

    if (evt.event === 'validation_error') {
      const msg = String(payload.message || 'validation_error');
      loops.set(msg, (loops.get(msg) || 0) + 1);
      if (current) current.validationErrors += 1;
      continue;
    }

    if (evt.event === 'system_error') {
      if (current) current.systemErrors += 1;
      continue;
    }

    if (evt.event === 'session_end') {
      sessionEndTs = tsMs;
    }
  }

  if (current && currentTs !== null && sessionEndTs !== null && sessionEndTs > currentTs) {
    current.dwellMs += sessionEndTs - currentTs;
  }

  const screenList = Array.from(screens.values());
  for (const s of screenList) {
    s.dwellSec = Math.round(s.dwellMs / 1000);
    s.frictionScore = (s.validationErrors * 4) + (s.systemErrors * 5) + (s.backtracks * 2) + Math.floor(s.dwellSec / 20);
  }

  screenList.sort((a, b) => b.frictionScore - a.frictionScore || b.dwellMs - a.dwellMs);

  const loopsList = Array.from(loops.entries())
    .map(([message, count]) => ({ message, count }))
    .filter((x) => x.count >= 2)
    .sort((a, b) => b.count - a.count);

  const longDwell = [...screenList]
    .sort((a, b) => b.dwellMs - a.dwellMs)
    .slice(0, 3);

  return {
    eventCount: events.length,
    screenCount: screenList.length,
    topFriction: screenList.slice(0, 3),
    loops: loopsList.slice(0, 3),
    longDwell,
  };
}

function formatReport(tracePath, analysis) {
  const lines = [];
  lines.push('# Agent Trace Friction Report');
  lines.push('');
  lines.push(`- Trace: \`${tracePath}\``);
  lines.push(`- Parsed events: ${analysis.eventCount}`);
  lines.push(`- Distinct screens: ${analysis.screenCount}`);
  lines.push('');

  lines.push('## Top 3 high-friction screens');
  if (!analysis.topFriction.length) {
    lines.push('- None');
  } else {
    for (const s of analysis.topFriction) {
      lines.push(`- ${s.id} (${s.title}): score=${s.frictionScore}, dwell=${s.dwellSec}s, validation_errors=${s.validationErrors}, system_errors=${s.systemErrors}, backtracks=${s.backtracks}`);
    }
  }
  lines.push('');

  lines.push('## Repeated error loops');
  if (!analysis.loops.length) {
    lines.push('- None detected');
  } else {
    for (const loop of analysis.loops) {
      lines.push(`- ${loop.count}x: ${loop.message}`);
    }
  }
  lines.push('');

  lines.push('## Longest dwell points');
  if (!analysis.longDwell.length) {
    lines.push('- None');
  } else {
    for (const s of analysis.longDwell) {
      lines.push(`- ${s.id} (${s.title}): ${s.dwellSec}s`);
    }
  }
  lines.push('');

  lines.push('## Suggested next checks');
  lines.push('- Re-run the same scripted journey after one UX copy/flow change.');
  lines.push('- Compare top-friction screens and dwell/error deltas run-over-run.');

  return `${lines.join('\n')}\n`;
}

(function main() {
  const { tracePath, outPath } = parseArgs(process.argv);
  if (!fs.existsSync(tracePath)) {
    console.error(`Trace file not found: ${tracePath}`);
    process.exit(1);
  }

  const text = fs.readFileSync(tracePath, 'utf8');
  const events = parseLines(text);
  const analysis = analyze(events);
  const report = formatReport(tracePath, analysis);

  if (outPath) {
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, report);
    console.log(`Wrote ${outPath}`);
    return;
  }

  process.stdout.write(report);
})();
