// Expected TS2322: provider-indexed keys cannot cross provider or projection version.
import type { ProviderKey,ProviderTypes } from '../../provider/indexed.ts';
import type { Brand,ProjectionVersion } from '../../ids.ts';
type A = ProviderTypes & {identity:Brand<'A','ProviderId'>;projectionVersion:ProjectionVersion};
type B = ProviderTypes & {identity:Brand<'B','ProviderId'>;projectionVersion:ProjectionVersion};
declare const b:ProviderKey<B>;
const forbidden:ProviderKey<A> = b;
type V1 = A & {projectionVersion:Brand<'1','ProjectionVersion'>};
type V2 = A & {projectionVersion:Brand<'2','ProjectionVersion'>};
declare const v2:ProviderKey<V2>;
const wrongVersion:ProviderKey<V1> = v2;
