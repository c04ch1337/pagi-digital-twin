
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { PagiProvider } from './context/PagiContext';
import { TelemetryProvider } from './context/TelemetryContext';
import { ThemeProvider } from './context/ThemeContext';

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
        <PagiProvider>
          <App />
        </PagiProvider>
      </TelemetryProvider>
    </ThemeProvider>
  </React.StrictMode>
);
