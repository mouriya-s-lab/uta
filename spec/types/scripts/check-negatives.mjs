import { readdirSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
const directory = new URL('../fixtures/negative/',import.meta.url);
const files = readdirSync(directory).filter(name=>name.endsWith('.ts')).sort();
if(files.length === 0) throw new Error('No negative fixtures discovered');
for(const file of files) {
 const source = readFileSync(new URL(file,directory),'utf8');
 const expected = source.match(/^\/\/ Expected (TS\d+):/m)?.[1];
 if(!expected) throw new Error(`${file}: missing Expected TS error header`);
 const result = spawnSync(process.execPath,['node_modules/typescript/bin/tsc','--noEmit','--strict','--exactOptionalPropertyTypes','--noUncheckedIndexedAccess','--target','ES2023','--module','ESNext','--moduleResolution','bundler','--allowImportingTsExtensions','--skipLibCheck',`fixtures/negative/${file}`],{encoding:'utf8'});
 if(result.error) throw result.error;
 const output = result.stdout+result.stderr;
 const codes = [...output.matchAll(/error (TS\d+):/g)].map(match=>match[1]);
 if(result.status === 0 || codes.length === 0 || codes.some(code=>code !== expected)) throw new Error(`${file}: expected only ${expected}, status ${result.status}\n${output}`);
 console.log(`PASS ${file}: ${expected} (${codes.length} diagnostic${codes.length === 1 ? '' : 's'})`);
}
console.log(`PASS ${files.length} negative fixtures`);
