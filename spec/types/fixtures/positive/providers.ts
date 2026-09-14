import { parseProviderId, parseProjectionVersion, parseOperationKind, parseSourceDigest, sha256 } from '../../ids.ts';
import { parseDuration } from '../../time.ts';
import { unwrap } from '../../result.ts';
import type { ProviderDeclaration } from '../../provider/declaration.ts';
import { providerDeclarationSchema } from '../../provider/declaration.ts';
const ms = (n:number)=>unwrap(parseDuration(n));
const version = unwrap(parseProjectionVersion('evidence-2026-09-14'));
const placeKind = unwrap(parseOperationKind('order.place'));
const observeKind = unwrap(parseOperationKind('order.observe'));
const sourceDigest = unwrap(parseSourceDigest(await sha256('UTA provider declaration evidence fixture')));
// These are evidence fixtures, not production release digests or writable Pack declarations.
export const alpaca:ProviderDeclaration = {
 providerId:unwrap(parseProviderId('alpaca')),projectionVersion:version,sourceDigest,transportFamily:['http'],auth:[],
 venues:{alpaca:{[placeKind]:{direction:'write',status:{kind:'unsupported',reason:'NoPlacementRecovery'},contract:{idempotency:{kind:'uniqueKey',keyField:'client_order_id',scope:'account',window:'unverified',reuseAfterTerminal:'unverified',duplicateResponse:'unverified'},readByKey:{kind:'byClientKey',operation:'getOrderByClientOrderId',coverage:'unverified',historyWindow:'unverified',ambiguity:'unique'}}},[observeKind]:{direction:'read',status:{kind:'supported'},cursor:{kind:'snapshotOnly'}}}}
};
export const longbridge:ProviderDeclaration = {
 providerId:unwrap(parseProviderId('longbridge')),projectionVersion:version,sourceDigest,transportFamily:['http'],auth:[],
 venues:{longbridge:{[placeKind]:{direction:'write',status:{kind:'unsupported',reason:'NoPlacementRecovery'},contract:{idempotency:{kind:'cachedReplay',keyField:'client_request_id',scope:'account',retention:ms(600000)},readByKey:{kind:'byProviderRef',coverage:'unverified',historyWindow:'unverified'}}},[observeKind]:{direction:'read',status:{kind:'supported'},cursor:{kind:'snapshotOnly'}}}}
};
export const okx:ProviderDeclaration = {
 providerId:unwrap(parseProviderId('okx')),projectionVersion:version,sourceDigest,transportFamily:['http'],auth:[],
 venues:{okx:{[placeKind]:{direction:'write',status:{kind:'unsupported',reason:'NoPlacementRecovery'},contract:{idempotency:{kind:'uniqueKey',keyField:'clOrdId',scope:'account',window:'untilTerminal',reuseAfterTerminal:true,duplicateResponse:'unverified'},readByKey:{kind:'byClientKey',operation:'getOrder',coverage:'openAndHistory',historyWindow:'unverified',ambiguity:'latestMatch'}}},[observeKind]:{direction:'read',status:{kind:'supported'},cursor:{kind:'snapshotOnly'}}}}
};
export const ibkrCP:ProviderDeclaration = {
 providerId:unwrap(parseProviderId('ibkr-cp')),projectionVersion:version,sourceDigest,transportFamily:['http'],auth:[],
 venues:{'ibkr-cp':{[placeKind]:{direction:'write',status:{kind:'unsupported',reason:'NoPlacementRecovery'},contract:{idempotency:{kind:'uniqueKey',keyField:'cOID',scope:'account',window:ms(86400000),reuseAfterTerminal:'unverified',duplicateResponse:'unverified'},readByKey:{kind:'byProviderRef',coverage:'unverified',historyWindow:'unverified'}}},[observeKind]:{direction:'read',status:{kind:'supported'},cursor:{kind:'snapshotOnly'}}}}
};
export const bybit:ProviderDeclaration = {
 providerId:unwrap(parseProviderId('bybit')),projectionVersion:version,sourceDigest,transportFamily:['http'],auth:[],
 venues:{bybit:{[placeKind]:{direction:'write',status:{kind:'unsupported',reason:'NoPlacementRecovery'},contract:{idempotency:{kind:'uniqueKey',keyField:'orderLinkId',scope:'account',window:'unverified',reuseAfterTerminal:'unverified',duplicateResponse:'rejectsWithCode'},readByKey:{kind:'byClientKey',operation:'getOrder',coverage:'openOnly',historyWindow:'unverified',ambiguity:'unique'}}},[observeKind]:{direction:'read',status:{kind:'supported'},cursor:{kind:'snapshotOnly'}}}}
};
for(const declaration of [alpaca,longbridge,okx,ibkrCP,bybit])providerDeclarationSchema.parse(declaration);
console.log('PASS providers: five non-activatable evidence declarations with synthetic source digest');
