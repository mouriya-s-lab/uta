// Expected TS2322: a supported write cannot declare no idempotency and no read recovery.
import type { CapabilityTable } from '../../provider/declaration.ts';
import type { OperationKind } from '../../ids.ts';
const forbidden:CapabilityTable[OperationKind] = {direction:'write',status:{kind:'supported'},contract:{idempotency:{kind:'none'},readByKey:{kind:'none'}}};
