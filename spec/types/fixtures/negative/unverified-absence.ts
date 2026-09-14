// Expected TS2322: unknown coverage/window cannot construct verified absence proof.
import type { RecoveryResult } from '../../provider/recovery.ts';
import type { ProviderTypes } from '../../provider/indexed.ts';
import type { Instant } from '../../ids.ts';
import type { Duration } from '../../time.ts';
declare const time:Instant;
declare const duration:Duration;
const forbidden:RecoveryResult<ProviderTypes> = {
 kind:'confirmedAbsent',validUntil:time,
 proof:{kind:'keyedLookupMiss',coverage:'openAndHistory',window:duration,attemptTime:time,checkedAt:time,
  contract:{kind:'byClientKey',operation:'lookup',coverage:'unverified',historyWindow:'unverified',ambiguity:'unique'}}
};
