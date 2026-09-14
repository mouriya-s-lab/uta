import type { ReadOnly, ReadWrite, Handlers, Heartbeat } from '../../channel.ts';
import type { LedgerEntry } from '../../ledger/entries.ts';
import type { EntryDraft } from '../../ledger/store.ts';
import { coreOperationKind,operationKindSchema } from '../../ids.ts';
import type { OperationKind,CoreOperationKind } from '../../ids.ts';
import { assertNever } from '../../result.ts';
import type { RuleResult } from '../../consumer.ts';
export type CompositionSpec = {ledger:ReadWrite<EntryDraft,readonly LedgerEntry[]>;heartbeat:ReadOnly<Heartbeat>};
export function compose(handlers:Handlers<CompositionSpec>):Handlers<CompositionSpec> { return handlers; }
// Real handlers are supplied by the composition root; a type assertion cannot fill a missing member.
export function compositionRoot(readLedger:()=>Promise<readonly LedgerEntry[]>,append:(entry:EntryDraft)=>Promise<readonly LedgerEntry[]>,heartbeat:()=>Promise<Heartbeat>):Handlers<CompositionSpec> {
 return compose({ledger:{read:readLedger,write:append},heartbeat:{read:heartbeat}});
}
function coreRule(kind:CoreOperationKind):RuleResult {
 switch(kind) {
  case 'order.place': return {kind:'allow'};
  case 'order.modify': return {kind:'allow'};
  case 'order.cancel': return {kind:'allow'};
  case 'position.close': return {kind:'allow'};
 }
 return assertNever(kind);
}
export function coreConsumerRule(kind:OperationKind):RuleResult {
 const known = coreOperationKind(kind);
 return known.kind === 'error' ? {kind:'reject',reason:known.error} : coreRule(known.value);
}
const unknownResult=coreConsumerRule(operationKindSchema.parse('extension.transfer'));
if(unknownResult.kind!=='reject'||unknownResult.reason.kind!=='UnknownKind')throw new Error('Core consumer must reject undeclared extension semantics');
if(coreConsumerRule(operationKindSchema.parse('order.place')).kind!=='allow')throw new Error('Known core operation must be consumed');
console.log('PASS composition: namespaced core rule boundary');
