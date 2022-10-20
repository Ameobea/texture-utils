import { browser } from '$app/environment';
import * as Sentry from '@sentry/browser';
import { Integrations } from '@sentry/tracing';

const sentryEnabled = () => browser && !window.location.href.includes('localhost');

export const maybeInitSentry = () => {
  if (sentryEnabled()) {
    Sentry.init({
      dsn: 'https://07a5f6465d5d49579848a1fddc4100b2@sentry.ameo.design/16',
      integrations: [new Integrations.BrowserTracing()],
      tracesSampleRate: 1.0,
    });
  }
};

export const getSentry = () => {
  if (!sentryEnabled()) {
    return null;
  }

  return Sentry;
};

export const captureMessage = (eventName: string, data?: any) =>
  getSentry()?.captureMessage(eventName, data ? { extra: data } : undefined);
