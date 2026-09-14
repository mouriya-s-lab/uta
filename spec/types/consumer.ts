import type { LedgerEntry,UnknownLedgerEntry,ReasonTree } from './ledger/entries.ts';
import type { LedgerPosition,Instant } from './ids.ts';
export type RuleResult={readonly kind:'allow'}|{readonly kind:'reject';readonly reason:ReasonTree}|{readonly kind:'pending';readonly reason:ReasonTree};
export type Consumer<K extends LedgerEntry['kind'],Context>={readonly name:string;readonly subscriptions:readonly K[];readonly checkpoint:LedgerPosition;readonly rule:(entry:Extract<LedgerEntry,{kind:K}>,context:Context,now:Instant)=>RuleResult;readonly onUnknown:(entry:UnknownLedgerEntry)=>RuleResult};
