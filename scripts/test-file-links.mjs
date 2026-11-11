import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/fileLinks.ts", import.meta.url), "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
}).outputText;
const { localFileReference } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);

test("local citations handle Windows paths, spaces, file URLs and source lines", () => {
  assert.deepEqual(localFileReference("C:/Documenti/Quiz finale.json"), { path: "C:/Documenti/Quiz finale.json", name: "Quiz finale.json" });
  assert.deepEqual(localFileReference("C:\\Documenti\\Quiz finale.json"), { path: "C:\\Documenti\\Quiz finale.json", name: "Quiz finale.json" });
  assert.deepEqual(localFileReference("file:///C:/Documenti/Quiz%20finale.json"), { path: "C:/Documenti/Quiz finale.json", name: "Quiz finale.json" });
  assert.deepEqual(localFileReference("/Users/test/slide%20finali.pptx"), { path: "/Users/test/slide finali.pptx", name: "slide finali.pptx" });
  assert.equal(localFileReference("C:/src/app.ts:23:4").path, "C:/src/app.ts");
  assert.equal(localFileReference("C:/src/app.ts#L23").path, "C:/src/app.ts");
});
test("web URLs, unsafe protocols, remote file hosts and malformed references are not local files", () => {
  for (const href of ["https://example.com/a.pdf", "javascript:alert(1)", "data:text/html,test", "file://server/share/a.txt", "//server/a.txt", "\\\\server\\a.txt", "relative.txt", "C:/bad%XX.txt", "C:/bad%00.txt", "C:/bad%0A.txt"]) {
    assert.equal(localFileReference(href), null, href);
  }
});
