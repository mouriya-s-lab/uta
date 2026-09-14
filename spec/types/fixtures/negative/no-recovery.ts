// Expected TS2322: none/none cannot construct a placement recovery witness.
import type { PlacementRecovery } from '../../provider/recovery.ts';
import type { ProviderTypes } from '../../provider/indexed.ts';
declare const recover:PlacementRecovery<ProviderTypes>['recover'];
const forbidden:PlacementRecovery<ProviderTypes> = {contract:{idempotency:{kind:'none'},readByKey:{kind:'none'}},recover};
