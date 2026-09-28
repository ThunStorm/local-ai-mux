import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const config = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url)));
const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url)));

test("desktop config points to the workspace frontend scripts", () => {
  assert.equal(config.productName, "LocalMux");
  assert.equal(config.identifier, "app.localmux.desktop");
  for (const key of ["beforeDevCommand", "beforeBuildCommand"]) {
    const script = config.build[key].split(" ")[1];
    assert.ok(Object.hasOwn(pkg.scripts, script), `${key} has matching script`);
  }
});
