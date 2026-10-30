// Unit tests for check-installer-size.mjs. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { LIMIT, isInstaller, megabytes, report } from "./check-installer-size.mjs";

test("installers and update bundles are checked; signatures and the manifest aren't", () => {
  for (const name of ["Splitr_0.1.0_x64_en-US.msi", "Splitr_0.1.0_x64-setup.exe", "Splitr_0.1.0_aarch64.dmg",
    "splitr_0.1.0_amd64.AppImage", "splitr_0.1.0_amd64.deb", "Splitr.app.tar.gz"]) {
    assert.equal(isInstaller(`/out/${name}`), true, name);
  }
  for (const name of ["Splitr_0.1.0_x64_en-US.msi.sig", "latest.json", "Splitr.app.tar.gz.sig", "Splitr.app"]) {
    assert.equal(isInstaller(`/out/${name}`), false, name);
  }
});

test("the limit is 25 MB and only larger installers fail", () => {
  assert.equal(LIMIT, 26214400);
  const { lines, tooBig } = report([
    ["a/Splitr.msi", LIMIT],
    ["a/Splitr.dmg", LIMIT + 1],
    ["a/Splitr.msi.sig", LIMIT * 2],
  ]);
  assert.deepEqual(tooBig, ["a/Splitr.dmg"]);
  assert.equal(lines.length, 2);
  assert.match(lines[0], /^ok +25\.0 MB {2}Splitr\.msi$/);
  assert.match(lines[1], /^TOO BIG +25\.0 MB {2}Splitr\.dmg$/);
});

test("a large AppImage is reported but doesn't fail, since it bundles WebKitGTK (SRS 8)", () => {
  const big = 83 * 1024 * 1024;
  const { lines, tooBig } = report([
    ["a/Splitr_0.1.0_amd64.AppImage", big],
    ["a/Splitr_0.1.0_amd64.deb", big],
  ]);
  assert.deepEqual(tooBig, ["a/Splitr_0.1.0_amd64.deb"]);
  assert.match(lines[0], /^note +83\.0 MB {2}Splitr_0\.1\.0_amd64\.AppImage {2}\(bundles WebKitGTK, SRS 8\)$/);
});

test("sizes read in megabytes", () => {
  assert.equal(megabytes(12.34 * 1024 * 1024), "12.3 MB");
});
