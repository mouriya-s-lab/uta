// Expected TS2345: the consumer requires Writer, but only Reader Layer is supplied.
// The only file permitted to import Effect; deliberately excluded from positive typecheck.
import { Context, Effect, Layer } from 'effect';
class Reader extends Context.Tag('Reader')<Reader,{readonly read:()=>number}>() {}
class Writer extends Context.Tag('Writer')<Writer,{readonly write:(value:number)=>void}>() {}
const program = Effect.gen(function*(){const reader = yield* Reader; const writer = yield* Writer; writer.write(reader.read());});
const readerLayer = Layer.succeed(Reader,{read:()=>1});
const incomplete = Effect.provide(program,readerLayer);
// exactOptionalPropertyTypes makes runPromise's object diagnostic TS2379.
// Check the actual residual Layer requirement directly to pin TS2345 instead.
declare const remaining:Effect.Effect.Context<typeof incomplete>;
function requireClosed(_remaining:never):void {}
requireClosed(remaining);
const complete = Effect.provide(program,Layer.merge(readerLayer,Layer.succeed(Writer,{write:(_value)=>undefined})));
Effect.runPromise(complete);
