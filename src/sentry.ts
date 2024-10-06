import { browser } from '$app/environment';
import * as Sentry from '@sentry/browser';

const sentryEnabled = () => browser && !window.location.href.includes('localhost');

export const maybeInitSentry = () => {
  if (sentryEnabled()) {
    Sentry.init({
      dsn: 'https://9414c7d9a937e7141f76e69b0ec69c68@sentry.ameo.design/11',
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
