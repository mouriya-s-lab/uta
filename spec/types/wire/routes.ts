import { z } from 'zod';
import {
  accountIdSchema,
  aliceIdSchema,
  asOfSchema,
  cursorSchema,
  idempotencyKeySchema,
  instantSchema,
  intentIdSchema,
  nativeKeySchema,
  proposalIdSchema,
  requestIdSchema,
  sourceSchema,
} from '../ids.ts';
import { decimalStringSchema } from '../money.ts';
import { durationSchema } from '../time.ts';
import {
  configEnvelopeSchema,
  observationEnvelopeSchema,
  operationEnvelopeSchema,
  providerOrderRefSchema,
  serializableSchema,
} from '../provider/indexed.ts';
import {
  authorizationDecisionRequestSchema,
  authorizationDecisionResponseSchema,
  commandResponseSchema,
  eventsQuerySchema,
  eventsResponseSchema,
  intentDetailResponseSchema,
  intentListResponseSchema,
  intentOutcomeSchema,
  intentProposalRequestSchema,
  intentProposalResponseSchema,
  ledgerEntriesQuerySchema,
  ledgerEntriesResponseSchema,
  oneShotIntentRequestSchema,
  proposalCreateRequestSchema,
  proposalCreateResponseSchema,
  readProjectionResponseSchema,
} from './projection.ts';
import {readinessResponseSchema} from '../readiness.ts';
import type {EventsQuery, EventsResponse, LedgerEntriesResponse} from './projection.ts';
import type {ReadinessResponse} from '../readiness.ts';

const emptyRequestSchema = z.strictObject({});
const opaqueSelectorSchema = z.string().min(1).max(512);
const boundedLimitSchema = z.number().int().min(1).max(2000);
const observationEnvelopeArraySchema = z.array(observationEnvelopeSchema);
const currencyCodeSchema = z.string().regex(/^[A-Z][A-Z0-9]{1,15}$/);
const brokerHealthSchema = z.strictObject({
  status: z.enum(['healthy', 'degraded', 'offline']),
  reach: z.enum(['down', 'connected', 'readable']),
  tier: z.enum(['data', 'account', 'trading']),
  consecutiveFailures: z.number().int().nonnegative(),
  lastError: z.string().min(1).optional(),
  lastSuccessAt: instantSchema.optional(),
  lastFailureAt: instantSchema.optional(),
  recovering: z.boolean(),
  connecting: z.boolean(),
  disabled: z.boolean(),
});
const historicalQualitySchema = z.enum(['realtime', 'iex', 'delayed', 'subscription']);
const historicalBarsCapabilitySchema = z.strictObject({
  supported: z.boolean(),
  quality: historicalQualitySchema.optional(),
  qualityBySecType: z.record(z.string().min(1), historicalQualitySchema).optional(),
  supportedBarSizes: z
    .array(z.enum(['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w']))
    .optional(),
});
const accountCapabilitiesSchema = z.strictObject({
  supportedSecTypes: z.array(z.string()),
  supportedOrderTypes: z.array(z.string()),
  historicalBars: historicalBarsCapabilitySchema.optional(),
});
export const utaSummarySchema = z.strictObject({
  id: accountIdSchema,
  label: z.string().min(1),
  asVendor: z.boolean(),
  capabilities: accountCapabilitiesSchema,
  health: brokerHealthSchema,
});
const brokerHealthStatusSchema = z.enum(['healthy', 'degraded', 'offline']);
export const equityAccountSchema = z.strictObject({
  id: accountIdSchema,
  label: z.string().min(1),
  baseCurrency: currencyCodeSchema,
  equity: decimalStringSchema,
  cash: decimalStringSchema,
  unrealizedPnL: decimalStringSchema,
  health: brokerHealthStatusSchema,
});
const contractSecTypeSchema = z.enum([
  'STK',
  'OPT',
  'FUT',
  'FOP',
  'IND',
  'CASH',
  'BOND',
  'CMDTY',
  'WAR',
  'IOPT',
  'FUND',
  'BAG',
  'NEWS',
  'CFD',
  'CRYPTO',
  'CRYPTO_PERP',
]);
const comboLegSchema = z.strictObject({
  conId: z.number().int().nonnegative(),
  ratio: decimalStringSchema,
  action: z.string(),
  exchange: z.string(),
  openClose: z.number().int(),
  shortSaleSlot: z.number().int(),
  designatedLocation: z.string(),
  exemptCode: z.number().int(),
});
const deltaNeutralContractSchema = z.strictObject({
  conId: z.number().int().nonnegative(),
  delta: decimalStringSchema,
  price: decimalStringSchema,
});
export const contractSearchContractSchema = z.strictObject({
  conId: z.number().int().nonnegative().optional(),
  symbol: z.string().optional(),
  secType: contractSecTypeSchema.optional(),
  lastTradeDateOrContractMonth: z.string().optional(),
  lastTradeDate: z.string().optional(),
  strike: decimalStringSchema.optional(),
  right: z.string().optional(),
  multiplier: decimalStringSchema.optional(),
  exchange: z.string().optional(),
  primaryExchange: z.string().optional(),
  currency: currencyCodeSchema.optional(),
  localSymbol: z.string().optional(),
  tradingClass: z.string().optional(),
  includeExpired: z.boolean().optional(),
  secIdType: z.string().optional(),
  secId: z.string().optional(),
  description: z.string().optional(),
  issuerId: z.string().optional(),
  comboLegsDescrip: z.string().optional(),
  comboLegs: z.array(comboLegSchema).optional(),
  deltaNeutralContract: deltaNeutralContractSchema.nullable().optional(),
});
export const contractSearchHitSchema = z.strictObject({
  aliceId: aliceIdSchema,
  nativeKey: nativeKeySchema,
  source: sourceSchema,
  contract: contractSearchContractSchema,
  derivativeSecTypes: z.array(z.string()),
  assetClass: z.enum(['equity', 'crypto', 'currency', 'commodity', 'unknown']).optional(),
});
export type ContractSearchHit = z.infer<typeof contractSearchHitSchema>;
export const fxRateFreshnessSchema = z.discriminatedUnion('kind', [
  z.strictObject({kind: z.literal('fresh')}),
  z.strictObject({kind: z.literal('stale'), age: durationSchema}),
  z.strictObject({kind: z.literal('missing')}),
]);
export type FxRateFreshness = z.infer<typeof fxRateFreshnessSchema>;
export const fxRateSchema = z.strictObject({
  base: currencyCodeSchema,
  quote: currencyCodeSchema,
  rate: decimalStringSchema,
  asOf: asOfSchema,
  source: sourceSchema,
});
export type FxRate = z.infer<typeof fxRateSchema>;
export const proposalOutcomeSchema = z.strictObject({
  proposalId: proposalIdSchema,
  members: z.array(
    z.strictObject({
      intentId: intentIdSchema,
      outcome: intentOutcomeSchema,
    }),
  ),
});
export const simulatorUtaSchema = z.strictObject({
  id: accountIdSchema,
  label: z.string().min(1),
});
const snapshotAccountSchema = z.strictObject({
  baseCurrency: currencyCodeSchema,
  netLiquidation: decimalStringSchema,
  totalCashValue: decimalStringSchema,
  unrealizedPnL: decimalStringSchema,
  realizedPnL: decimalStringSchema,
  buyingPower: decimalStringSchema.optional(),
  initMarginReq: decimalStringSchema.optional(),
  maintMarginReq: decimalStringSchema.optional(),
});
const snapshotPositionSchema = z.strictObject({
  aliceId: aliceIdSchema,
  currency: currencyCodeSchema,
  side: z.enum(['long', 'short']),
  quantity: decimalStringSchema,
  avgCost: decimalStringSchema,
  marketPrice: decimalStringSchema,
  marketValue: decimalStringSchema,
  unrealizedPnL: decimalStringSchema,
  realizedPnL: decimalStringSchema,
  secType: z.string().optional(),
  multiplier: decimalStringSchema.optional(),
  strike: decimalStringSchema.optional(),
  right: z.string().optional(),
  expiry: z.string().optional(),
});
const snapshotOpenOrderSchema = z.strictObject({
  orderId: z.string().min(1),
  aliceId: aliceIdSchema,
  action: z.string().min(1),
  orderType: z.string().min(1),
  totalQuantity: decimalStringSchema,
  limitPrice: decimalStringSchema.optional(),
  status: z.string().min(1),
  avgFillPrice: decimalStringSchema.optional(),
});
export const snapshotSchema = z.strictObject({
  accountId: accountIdSchema,
  timestamp: instantSchema,
  trigger: z.enum(['scheduled', 'post-push', 'post-reject', 'manual']),
  account: snapshotAccountSchema,
  positions: z.array(snapshotPositionSchema),
  openOrders: z.array(snapshotOpenOrderSchema),
  health: z.enum(['healthy', 'degraded', 'offline', 'disabled']),
  headCommit: z.string().min(1).nullable(),
  pendingCommits: z.array(z.string().min(1)),
});
export const equityCurvePointSchema = z.strictObject({
  timestamp: instantSchema,
  equity: decimalStringSchema,
  accounts: z.record(accountIdSchema, decimalStringSchema),
});

/** GET /v2/readiness */
export const getReadinessRequestSchema = emptyRequestSchema;
export type GetReadinessRequest = z.infer<typeof getReadinessRequestSchema>;
export const getReadinessResponseSchema = readinessResponseSchema;
export type GetReadinessResponse = ReadinessResponse;

/** GET /v2/events */
export const listEventsRequestSchema = eventsQuerySchema;
export type ListEventsRequest = EventsQuery;
export const listEventsResponseSchema = eventsResponseSchema;
export type ListEventsResponse = EventsResponse;

/** GET /v2/openapi.json */
export const getOpenApiRequestSchema = emptyRequestSchema;
export type GetOpenApiRequest = z.infer<typeof getOpenApiRequestSchema>;
export const openApiDocumentSchema = z
  .object({
    openapi: z.string().startsWith('3.1'),
    paths: z.record(z.string(), serializableSchema),
  })
  .passthrough();
export const getOpenApiResponseSchema = openApiDocumentSchema;
export type OpenApiDocument = z.infer<typeof openApiDocumentSchema>;
export type GetOpenApiResponse = z.infer<typeof getOpenApiResponseSchema>;

/** GET /v2/utas */
const utaListValueSchema = z.strictObject({utas: z.array(utaSummarySchema)});
export const listUtasRequestSchema = emptyRequestSchema;
export type ListUtasRequest = z.infer<typeof listUtasRequestSchema>;
export const listUtasResponseSchema = readProjectionResponseSchema(utaListValueSchema);
export type ListUtasResponse = z.infer<typeof listUtasResponseSchema>;

/** GET /v2/equity */
export const equityValueSchema = z.strictObject({
  totalEquity: decimalStringSchema,
  totalCash: decimalStringSchema,
  totalUnrealizedPnL: decimalStringSchema,
  totalRealizedPnL: decimalStringSchema,
  fxWarnings: z.array(z.string().min(1)).optional(),
  accounts: z.array(equityAccountSchema),
});
export const getEquityResponseSchema = readProjectionResponseSchema(equityValueSchema);
export type GetEquityResponse = z.infer<typeof getEquityResponseSchema>;

/** GET /v2/contracts/search */
export const searchContractsRequestSchema = z
  .strictObject({
    pattern: opaqueSelectorSchema.optional(),
    query: opaqueSelectorSchema.optional(),
    assetClass: opaqueSelectorSchema.optional(),
    source: sourceSchema.optional(),
    accountId: accountIdSchema.optional(),
  })
  .refine((value) => value.pattern !== undefined || value.query !== undefined, {
    message: 'pattern or query is required',
  });
export type SearchContractsRequest = z.infer<typeof searchContractsRequestSchema>;
export const searchContractsValueSchema = z.strictObject({
  results: z.array(contractSearchHitSchema),
  count: z.number().int().nonnegative(),
  utasConfigured: z.number().int().nonnegative().optional(),
});
export const searchContractsResponseSchema = readProjectionResponseSchema(searchContractsValueSchema);
export type SearchContractsResponse = z.infer<typeof searchContractsResponseSchema>;

/** GET /v2/fx-rates */
export const fxRatesValueSchema = z.strictObject({rates: z.array(fxRateSchema)});
export const getFxRatesRequestSchema = emptyRequestSchema;
export type GetFxRatesRequest = z.infer<typeof getFxRatesRequestSchema>;
export const getFxRatesResponseSchema = z.strictObject({
  source: sourceSchema,
  asOf: asOfSchema,
  freshness: fxRateFreshnessSchema,
  value: fxRatesValueSchema,
});
export type GetFxRatesResponse = z.infer<typeof getFxRatesResponseSchema>;

/** POST /v2/test-connection */
export const testConnectionRequestSchema = z.strictObject({
  accountId: accountIdSchema.optional(),
  config: configEnvelopeSchema,
});
export type TestConnectionRequest = z.infer<typeof testConnectionRequestSchema>;
const testConnectionValueSchema = z.strictObject({
  success: z.boolean(),
  account: observationEnvelopeSchema.optional(),
  positions: observationEnvelopeArraySchema.optional(),
});
export const testConnectionResponseSchema = readProjectionResponseSchema(testConnectionValueSchema);
export type TestConnectionResponse = z.infer<typeof testConnectionResponseSchema>;

/** Account observation requests */
export const listSubaccountsRequestSchema = emptyRequestSchema;
export type ListSubaccountsRequest = z.infer<typeof listSubaccountsRequestSchema>;
export const listSubaccountsResponseSchema = readProjectionResponseSchema(
  z.strictObject({subAccounts: observationEnvelopeArraySchema}),
);
export type ListSubaccountsResponse = z.infer<typeof listSubaccountsResponseSchema>;

export const getAccountRequestSchema = z.strictObject({
  subAccountId: opaqueSelectorSchema.optional(),
});
export type GetAccountRequest = z.infer<typeof getAccountRequestSchema>;
export const getAccountResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type GetAccountResponse = z.infer<typeof getAccountResponseSchema>;

export const listPositionsRequestSchema = z.strictObject({
  subAccountId: opaqueSelectorSchema.optional(),
});
export type ListPositionsRequest = z.infer<typeof listPositionsRequestSchema>;
export const listPositionsResponseSchema = readProjectionResponseSchema(
  z.strictObject({positions: observationEnvelopeArraySchema}),
);
export type ListPositionsResponse = z.infer<typeof listPositionsResponseSchema>;

export const listOrdersRequestSchema = z.strictObject({
  ids: z.string().min(1).optional(),
});
export type ListOrdersRequest = z.infer<typeof listOrdersRequestSchema>;
export const listOrdersResponseSchema = readProjectionResponseSchema(
  z.strictObject({orders: observationEnvelopeArraySchema}),
);
export type ListOrdersResponse = z.infer<typeof listOrdersResponseSchema>;

export const getMarketClockRequestSchema = emptyRequestSchema;
export type GetMarketClockRequest = z.infer<typeof getMarketClockRequestSchema>;
export const getMarketClockResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type GetMarketClockResponse = z.infer<typeof getMarketClockResponseSchema>;

/** POST /v2/accounts/{accountId}/quote */
export const getQuoteRequestSchema = z.strictObject({
  contract: operationEnvelopeSchema,
});
export type GetQuoteRequest = z.infer<typeof getQuoteRequestSchema>;
export const getQuoteResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type GetQuoteResponse = z.infer<typeof getQuoteResponseSchema>;

/** POST /v2/accounts/{accountId}/research/* */
export const researchOptionContractsRequestSchema = z.strictObject({
  aliceId: aliceIdSchema.optional(),
  contract: operationEnvelopeSchema.optional(),
  params: operationEnvelopeSchema,
  cursor: cursorSchema.optional(),
});
export type ResearchOptionContractsRequest = z.infer<typeof researchOptionContractsRequestSchema>;
export const researchOptionContractsResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type ResearchOptionContractsResponse = z.infer<typeof researchOptionContractsResponseSchema>;

export const researchOptionChainRequestSchema = researchOptionContractsRequestSchema;
export type ResearchOptionChainRequest = ResearchOptionContractsRequest;
export const researchOptionChainResponseSchema = researchOptionContractsResponseSchema;
export type ResearchOptionChainResponse = ResearchOptionContractsResponse;

export const researchOrderBookRequestSchema = researchOptionContractsRequestSchema;
export type ResearchOrderBookRequest = ResearchOptionContractsRequest;
export const researchOrderBookResponseSchema = researchOptionContractsResponseSchema;
export type ResearchOrderBookResponse = ResearchOptionContractsResponse;

/** Other contract observations */
export const expandContractsRequestSchema = z.strictObject({
  aliceId: aliceIdSchema,
  filters: configEnvelopeSchema.optional(),
});
export type ExpandContractsRequest = z.infer<typeof expandContractsRequestSchema>;
export const expandContractsResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type ExpandContractsResponse = z.infer<typeof expandContractsResponseSchema>;

export const getHistoricalRequestSchema = z.strictObject({
  contract: operationEnvelopeSchema,
  params: z
    .strictObject({
      startTime: instantSchema,
      endTime: instantSchema,
      interval: opaqueSelectorSchema.optional(),
    })
    .refine((value) => value.endTime >= value.startTime, 'endTime must be at least startTime'),
});
export type GetHistoricalRequest = z.infer<typeof getHistoricalRequestSchema>;
export const getHistoricalResponseSchema = readProjectionResponseSchema(
  z.strictObject({bars: observationEnvelopeArraySchema}),
);
export type GetHistoricalResponse = z.infer<typeof getHistoricalResponseSchema>;

export const getContractDetailsRequestSchema = z.strictObject({
  contract: operationEnvelopeSchema,
  aliceId: aliceIdSchema.optional(),
});
export type GetContractDetailsRequest = z.infer<typeof getContractDetailsRequestSchema>;
export const getContractDetailsResponseSchema = readProjectionResponseSchema(
  observationEnvelopeSchema.nullable(),
);
export type GetContractDetailsResponse = z.infer<typeof getContractDetailsResponseSchema>;
export const listLedgerEntriesRequestSchema = ledgerEntriesQuerySchema;
export type ListLedgerEntriesRequest = z.infer<typeof listLedgerEntriesRequestSchema>;
export const listLedgerEntriesResponseSchema = ledgerEntriesResponseSchema;
export type ListLedgerEntriesResponse = LedgerEntriesResponse;

export const listOrderHistoryRequestSchema = z.strictObject({
  limit: boundedLimitSchema.optional(),
});
export type ListOrderHistoryRequest = z.infer<typeof listOrderHistoryRequestSchema>;
export const listOrderHistoryResponseSchema = readProjectionResponseSchema(
  z.strictObject({orders: observationEnvelopeArraySchema}),
);
export type ListOrderHistoryResponse = z.infer<typeof listOrderHistoryResponseSchema>;

export const listTradeHistoryRequestSchema = z.strictObject({
  limit: boundedLimitSchema.optional(),
});
export type ListTradeHistoryRequest = z.infer<typeof listTradeHistoryRequestSchema>;
export const listTradeHistoryResponseSchema = readProjectionResponseSchema(
  z.strictObject({trades: observationEnvelopeArraySchema}),
);
export type ListTradeHistoryResponse = z.infer<typeof listTradeHistoryResponseSchema>;

export const getArchiveHistoryRequestSchema = ledgerEntriesQuerySchema;
export type GetArchiveHistoryRequest = z.infer<typeof getArchiveHistoryRequestSchema>;
export const getArchiveHistoryResponseSchema = ledgerEntriesResponseSchema;
export type GetArchiveHistoryResponse = LedgerEntriesResponse;

/** Intent, proposal, decision, reconcile and lifecycle */
export const listIntentsRequestSchema = z.strictObject({
  state: z
    .enum([
      'proposed',
      'authorized',
      'withdrawn',
      'rejected',
      'expired',
      'attempting',
      'accepted',
      'rejectedByProvider',
      'unknown',
      'awaitingReview',
      'filled',
      'partiallyFilled',
      'cancelled',
      'abandoned',
    ])
    .optional(),
  proposalId: proposalIdSchema.optional(),
});
export type ListIntentsRequest = z.infer<typeof listIntentsRequestSchema>;
export const listIntentsResponseSchema = intentListResponseSchema;
export type ListIntentsResponse = z.infer<typeof listIntentsResponseSchema>;

export const getIntentRequestSchema = emptyRequestSchema;
export type GetIntentRequest = z.infer<typeof getIntentRequestSchema>;
export const getIntentResponseSchema = intentDetailResponseSchema;
export type GetIntentResponse = z.infer<typeof getIntentResponseSchema>;

export const listProposalsRequestSchema = z.strictObject({
  state: opaqueSelectorSchema.optional(),
});
export type ListProposalsRequest = z.infer<typeof listProposalsRequestSchema>;
const proposalListValueSchema = z.strictObject({proposals: z.array(proposalOutcomeSchema)});
export const listProposalsResponseSchema = readProjectionResponseSchema(proposalListValueSchema);
export type ListProposalsResponse = z.infer<typeof listProposalsResponseSchema>;

export const createProposalRequestSchema = proposalCreateRequestSchema;
export type CreateProposalRequest = z.infer<typeof createProposalRequestSchema>;
export const createProposalResponseSchema = proposalCreateResponseSchema;
export type CreateProposalResponse = z.infer<typeof createProposalResponseSchema>;

export const proposeIntentRequestSchema = intentProposalRequestSchema;
export type ProposeIntentRequest = z.infer<typeof proposeIntentRequestSchema>;
export const proposeIntentResponseSchema = intentProposalResponseSchema;
export type ProposeIntentResponse = z.infer<typeof proposeIntentResponseSchema>;

export const decideIntentRequestSchema = authorizationDecisionRequestSchema;
export type DecideIntentRequest = z.infer<typeof decideIntentRequestSchema>;
export const decideIntentResponseSchema = authorizationDecisionResponseSchema;
export type DecideIntentResponse = z.infer<typeof decideIntentResponseSchema>;

export const proposeAndDecideIntentRequestSchema = oneShotIntentRequestSchema;
export type ProposeAndDecideIntentRequest = z.infer<typeof proposeAndDecideIntentRequestSchema>;
export const proposeAndDecideIntentResponseSchema = intentProposalResponseSchema;
export type ProposeAndDecideIntentResponse = z.infer<typeof proposeAndDecideIntentResponseSchema>;

export const requestReconciliationRequestSchema = z.strictObject({
  requestId: requestIdSchema,
  idempotencyKey: idempotencyKeySchema,
  scope: z.union([
    z.strictObject({kind: z.literal('proposal'), proposalId: proposalIdSchema}),
    z.strictObject({kind: z.literal('intents'), intentIds: z.array(intentIdSchema).min(1)}),
  ]),
});
export type RequestReconciliationRequest = z.infer<typeof requestReconciliationRequestSchema>;
export const requestReconciliationResponseSchema = commandResponseSchema;
export type RequestReconciliationResponse = z.infer<typeof requestReconciliationResponseSchema>;

export const reconnectAccountRequestSchema = z.strictObject({requestId: requestIdSchema});
export type ReconnectAccountRequest = z.infer<typeof reconnectAccountRequestSchema>;
export const reconnectAccountResponseSchema = commandResponseSchema;
export type ReconnectAccountResponse = z.infer<typeof reconnectAccountResponseSchema>;

/** Simulator */
export const listSimulatorUtasRequestSchema = emptyRequestSchema;
export type ListSimulatorUtasRequest = z.infer<typeof listSimulatorUtasRequestSchema>;
export const listSimulatorUtasResponseSchema = readProjectionResponseSchema(
  z.strictObject({utas: z.array(simulatorUtaSchema)}),
);
export type ListSimulatorUtasResponse = z.infer<typeof listSimulatorUtasResponseSchema>;

export const getSimulatorStateRequestSchema = emptyRequestSchema;
export type GetSimulatorStateRequest = z.infer<typeof getSimulatorStateRequestSchema>;
export const getSimulatorStateResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type GetSimulatorStateResponse = z.infer<typeof getSimulatorStateResponseSchema>;

export const simulatePriceRequestSchema = z.strictObject({
  requestId: requestIdSchema,
  changes: operationEnvelopeSchema,
});
export type SimulatePriceRequest = z.infer<typeof simulatePriceRequestSchema>;
export const simulatePriceResponseSchema = readProjectionResponseSchema(observationEnvelopeSchema);
export type SimulatePriceResponse = z.infer<typeof simulatePriceResponseSchema>;

export const executeSimulatorActionRequestSchema = z.discriminatedUnion('action', [
  z.strictObject({action: z.literal('mark'), requestId: requestIdSchema, nativeKey: nativeKeySchema, price: decimalStringSchema}),
  z.strictObject({action: z.literal('tick'), requestId: requestIdSchema, nativeKey: nativeKeySchema, deltaPercent: decimalStringSchema}),
  z.strictObject({action: z.literal('fill'), requestId: requestIdSchema, providerRef: providerOrderRefSchema, qty: decimalStringSchema.optional(), price: decimalStringSchema.optional()}),
  z.strictObject({action: z.literal('cancel'), requestId: requestIdSchema, providerRef: providerOrderRefSchema}),
  z.strictObject({action: z.literal('deposit'), requestId: requestIdSchema, nativeKey: nativeKeySchema, quantity: decimalStringSchema, contract: operationEnvelopeSchema.optional()}),
  z.strictObject({action: z.literal('withdraw'), requestId: requestIdSchema, nativeKey: nativeKeySchema, quantity: decimalStringSchema}),
  z.strictObject({action: z.literal('trade'), requestId: requestIdSchema, nativeKey: nativeKeySchema, quantity: decimalStringSchema, price: decimalStringSchema, side: z.enum(['BUY', 'SELL']), contract: operationEnvelopeSchema.optional()}),
]);
export type ExecuteSimulatorActionRequest = z.infer<typeof executeSimulatorActionRequestSchema>;
export const executeSimulatorActionResponseSchema = commandResponseSchema;
export type ExecuteSimulatorActionResponse = z.infer<typeof executeSimulatorActionResponseSchema>;

/** Snapshots and equity curve */
export const listSnapshotsRequestSchema = z.strictObject({
  limit: boundedLimitSchema.optional(),
  startTime: instantSchema.optional(),
  endTime: instantSchema.optional(),
});
export type ListSnapshotsRequest = z.infer<typeof listSnapshotsRequestSchema>;
export const listSnapshotsResponseSchema = readProjectionResponseSchema(
  z.strictObject({snapshots: z.array(snapshotSchema)}),
);
export type ListSnapshotsResponse = z.infer<typeof listSnapshotsResponseSchema>;

export const deleteSnapshotRequestSchema = z.strictObject({
  requestId: requestIdSchema,
});
export type DeleteSnapshotRequest = z.infer<typeof deleteSnapshotRequestSchema>;
export const deleteSnapshotResponseSchema = commandResponseSchema;
export type DeleteSnapshotResponse = z.infer<typeof deleteSnapshotResponseSchema>;

export const getEquityCurveRequestSchema = z.strictObject({
  limit: boundedLimitSchema.optional(),
  startTime: instantSchema.optional(),
  endTime: instantSchema.optional(),
});
export type GetEquityCurveRequest = z.infer<typeof getEquityCurveRequestSchema>;
export const getEquityCurveResponseSchema = readProjectionResponseSchema(
  z.strictObject({points: z.array(equityCurvePointSchema)}),
);
export type GetEquityCurveResponse = z.infer<typeof getEquityCurveResponseSchema>;

/** GET /__uta/health */
export const getHealthRequestSchema = emptyRequestSchema;
export type GetHealthRequest = z.infer<typeof getHealthRequestSchema>;
export const healthResponseSchema = z.strictObject({
  ok: z.literal(true),
  startedAt: z.string().min(1),
  utas: z.number().int().nonnegative(),
});
export type HealthResponse = z.infer<typeof healthResponseSchema>;
export const getHealthResponseSchema = healthResponseSchema;
export type GetHealthResponse = HealthResponse;


export type RouteChannel = 'read-only' | 'read-write';
export type RouteKind = 'trading' | 'simulator' | 'lifecycle';
export type RoutePrincipal = 'bearer-only' | 'bearer+principal';
export type HttpMethod = 'GET' | 'POST' | 'DELETE';
export type RouteSchema = z.ZodType<unknown>;
export type RouteDefinition = {
  readonly method: HttpMethod;
  readonly path: string;
  readonly channel: RouteChannel;
  readonly kind: RouteKind;
  readonly principal: RoutePrincipal;
  readonly params?: RouteSchema;
  readonly request?: RouteSchema;
  readonly response: RouteSchema;
  readonly statuses: readonly number[];
};

export const accountRouteParamsSchema = z.strictObject({accountId: accountIdSchema});
export type AccountRouteParams = z.infer<typeof accountRouteParamsSchema>;
export const intentRouteParamsSchema = z.strictObject({intentId: intentIdSchema});
export type IntentRouteParams = z.infer<typeof intentRouteParamsSchema>;
export const archiveRouteParamsSchema = z.strictObject({accountId: accountIdSchema, selector: opaqueSelectorSchema});
export type ArchiveRouteParams = z.infer<typeof archiveRouteParamsSchema>;
export const snapshotRouteParamsSchema = z.strictObject({accountId: accountIdSchema, timestamp: instantSchema});
export type SnapshotRouteParams = z.infer<typeof snapshotRouteParamsSchema>;

const statuses = {
  readiness: [200, 401, 503] as const,
  events: [200, 400, 401, 404, 500, 503, 504] as const,
  openapi: [200, 401] as const,
  providerRead: [200, 400, 401, 404, 422, 502, 503, 504] as const,
  globalRead: [200, 401, 500, 503, 504] as const,
  localRead: [200, 400, 401, 404, 500, 503] as const,
  intentProposal: [200, 201, 400, 401, 404, 409, 422, 500, 503, 504] as const,
  decision: [200, 202, 400, 401, 404, 409, 422, 500, 503, 504] as const,
  proposalCommand: [200, 201, 400, 401, 404, 409, 500, 503, 504] as const,
  reconcileCommand: [200, 202, 400, 401, 404, 409, 422, 500, 503, 504] as const,
  lifecycleCommand: [200, 202, 400, 401, 404, 409, 500, 503, 504] as const,
  snapshotDelete: [200, 400, 401, 404, 409, 500, 503, 504] as const,
  simulatorRead: [200, 400, 401, 404, 500, 503, 504] as const,
  simulatorPrice: [200, 400, 401, 404, 422, 500, 504] as const,
  simulatorAction: [200, 202, 400, 401, 404, 409, 422, 500, 503, 504] as const,
  health: [200, 401] as const,
} as const;

export const ROUTES = [
  {method: 'GET', path: '/v2/readiness', channel: 'read-only', kind: 'lifecycle', principal: 'bearer-only', response: getReadinessResponseSchema, statuses: statuses.readiness},
  {method: 'GET', path: '/v2/events', channel: 'read-only', kind: 'lifecycle', principal: 'bearer+principal', request: listEventsRequestSchema, response: listEventsResponseSchema, statuses: statuses.events},
  {method: 'GET', path: '/v2/openapi.json', channel: 'read-only', kind: 'lifecycle', principal: 'bearer-only', response: getOpenApiResponseSchema, statuses: statuses.openapi},
  {method: 'GET', path: '/v2/utas', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', response: listUtasResponseSchema, statuses: statuses.globalRead},
  {method: 'GET', path: '/v2/equity', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', response: getEquityResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/contracts/search', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', request: searchContractsRequestSchema, response: searchContractsResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/fx-rates', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', response: getFxRatesResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/test-connection', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', request: testConnectionRequestSchema, response: testConnectionResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/subaccounts', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listSubaccountsRequestSchema, response: listSubaccountsResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/accounts/{accountId}', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: getAccountRequestSchema, response: getAccountResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/positions', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listPositionsRequestSchema, response: listPositionsResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/orders', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listOrdersRequestSchema, response: listOrdersResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/market-clock', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: getMarketClockRequestSchema, response: getMarketClockResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/quote', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: getQuoteRequestSchema, response: getQuoteResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/research/option-contracts', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: researchOptionContractsRequestSchema, response: researchOptionContractsResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/research/option-chain', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: researchOptionChainRequestSchema, response: researchOptionChainResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/research/order-book', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: researchOrderBookRequestSchema, response: researchOrderBookResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/contracts/expand', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: expandContractsRequestSchema, response: expandContractsResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/historical', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: getHistoricalRequestSchema, response: getHistoricalResponseSchema, statuses: statuses.providerRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/contracts/details', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: getContractDetailsRequestSchema, response: getContractDetailsResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/ledger/entries', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listLedgerEntriesRequestSchema, response: listLedgerEntriesResponseSchema, statuses: statuses.localRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/history/orders', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listOrderHistoryRequestSchema, response: listOrderHistoryResponseSchema, statuses: statuses.localRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/history/trades', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listTradeHistoryRequestSchema, response: listTradeHistoryResponseSchema, statuses: statuses.localRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/history/archive/{selector}', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: archiveRouteParamsSchema, request: getArchiveHistoryRequestSchema, response: getArchiveHistoryResponseSchema, statuses: statuses.localRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/intents', channel: 'read-write', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: proposeIntentRequestSchema, response: proposeIntentResponseSchema, statuses: statuses.intentProposal},
  {method: 'GET', path: '/v2/accounts/{accountId}/intents', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listIntentsRequestSchema, response: listIntentsResponseSchema, statuses: statuses.localRead},
  {method: 'GET', path: '/v2/intents/{intentId}', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: intentRouteParamsSchema, request: getIntentRequestSchema, response: getIntentResponseSchema, statuses: statuses.localRead},
  {method: 'GET', path: '/v2/accounts/{accountId}/proposals', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listProposalsRequestSchema, response: listProposalsResponseSchema, statuses: statuses.localRead},
  {method: 'POST', path: '/v2/accounts/{accountId}/proposals', channel: 'read-write', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: createProposalRequestSchema, response: createProposalResponseSchema, statuses: statuses.proposalCommand},
  {method: 'POST', path: '/v2/intents/{intentId}/decisions', channel: 'read-write', kind: 'trading', principal: 'bearer+principal', params: intentRouteParamsSchema, request: decideIntentRequestSchema, response: decideIntentResponseSchema, statuses: statuses.decision},
  {method: 'POST', path: '/v2/accounts/{accountId}/intents/one-shot', channel: 'read-write', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: proposeAndDecideIntentRequestSchema, response: proposeAndDecideIntentResponseSchema, statuses: statuses.decision},
  {method: 'POST', path: '/v2/accounts/{accountId}/reconciliations', channel: 'read-write', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: requestReconciliationRequestSchema, response: requestReconciliationResponseSchema, statuses: statuses.reconcileCommand},
  {method: 'POST', path: '/v2/accounts/{accountId}/lifecycle/reconnect', channel: 'read-write', kind: 'lifecycle', principal: 'bearer+principal', params: accountRouteParamsSchema, request: reconnectAccountRequestSchema, response: reconnectAccountResponseSchema, statuses: statuses.lifecycleCommand},
  {method: 'GET', path: '/v2/simulator/utas', channel: 'read-only', kind: 'simulator', principal: 'bearer+principal', response: listSimulatorUtasResponseSchema, statuses: statuses.simulatorRead},
  {method: 'GET', path: '/v2/simulator/accounts/{accountId}/state', channel: 'read-only', kind: 'simulator', principal: 'bearer+principal', params: accountRouteParamsSchema, request: getSimulatorStateRequestSchema, response: getSimulatorStateResponseSchema, statuses: statuses.simulatorRead},
  {method: 'POST', path: '/v2/simulator/accounts/{accountId}/price-simulation', channel: 'read-only', kind: 'simulator', principal: 'bearer+principal', params: accountRouteParamsSchema, request: simulatePriceRequestSchema, response: simulatePriceResponseSchema, statuses: statuses.simulatorPrice},
  {method: 'POST', path: '/v2/simulator/accounts/{accountId}/actions', channel: 'read-write', kind: 'simulator', principal: 'bearer+principal', params: accountRouteParamsSchema, request: executeSimulatorActionRequestSchema, response: executeSimulatorActionResponseSchema, statuses: statuses.simulatorAction},
  {method: 'GET', path: '/v2/accounts/{accountId}/snapshots', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', params: accountRouteParamsSchema, request: listSnapshotsRequestSchema, response: listSnapshotsResponseSchema, statuses: statuses.providerRead},
  {method: 'DELETE', path: '/v2/accounts/{accountId}/snapshots/{timestamp}', channel: 'read-write', kind: 'lifecycle', principal: 'bearer+principal', params: snapshotRouteParamsSchema, request: deleteSnapshotRequestSchema, response: deleteSnapshotResponseSchema, statuses: statuses.snapshotDelete},
  {method: 'GET', path: '/v2/equity-curve', channel: 'read-only', kind: 'trading', principal: 'bearer+principal', request: getEquityCurveRequestSchema, response: getEquityCurveResponseSchema, statuses: statuses.providerRead},
  {method: 'GET', path: '/__uta/health', channel: 'read-only', kind: 'lifecycle', principal: 'bearer-only', response: getHealthResponseSchema, statuses: statuses.health},
] as const satisfies readonly RouteDefinition[];
