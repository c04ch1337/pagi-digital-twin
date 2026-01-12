
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { PagiProvider } from './context/PagiContext';
import { TelemetryProvider } from './context/TelemetryContext';

import './index.css';

const rootElement = document.getElementById('root');
if (!rootElement) {
  throw new Error("Could not find root element to mount to");
}

const root = ReactDOM.createRoot(rootElement);
root.render(
  <React.StrictMode>
    <TelemetryProvider>
      <PagiProvider>
        <App />
      </PagiProvider>
    </TelemetryProvider>
  </React.StrictMode>
);
