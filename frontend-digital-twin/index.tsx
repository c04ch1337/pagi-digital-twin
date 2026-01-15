
import React from 'react';
import ReactDOM from 'react-dom/client';
import { Toaster } from 'react-hot-toast';
import App from './App';
import { PagiProvider } from './context/PagiContext';
import { TelemetryProvider } from './context/TelemetryContext';
import { ThemeProvider } from './context/ThemeContext';
import { DomainAttributionProvider } from './context/DomainAttributionContext';

import './index.css';

const rootElement = document.getElementById('root');
if (!rootElement) {
  throw new Error("Could not find root element to mount to");
}

const root = ReactDOM.createRoot(rootElement);
root.render(
  <React.StrictMode>
    <ThemeProvider>
      <TelemetryProvider>
        <DomainAttributionProvider>
          <PagiProvider>
            <App />
            <Toaster
              position="top-right"
              toastOptions={{
                duration: 4000,
                style: {
                  background: 'rgb(var(--surface-rgb))',
                  color: 'var(--text-primary)',
                  border: '1px solid rgb(var(--bg-steel-rgb)/0.3)',
                },
                success: {
                  iconTheme: {
                    primary: 'rgb(var(--success-rgb))',
                    secondary: 'var(--text-primary)',
                  },
                },
                error: {
                  iconTheme: {
                    primary: 'rgb(var(--danger-rgb))',
                    secondary: 'var(--text-primary)',
                  },
                },
              }}
            />
          </PagiProvider>
        </DomainAttributionProvider>
      </TelemetryProvider>
    </ThemeProvider>
  </React.StrictMode>
);
