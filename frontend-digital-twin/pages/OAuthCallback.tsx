import React, { useEffect, useState } from 'react';
import { setOAuthTokens } from '../services/emailTeamsService';

/**
 * OAuth callback handler for Microsoft Graph API authentication
 * Handles the redirect from Microsoft OAuth flow and exchanges authorization code for tokens
 */
const OAuthCallback: React.FC = () => {
  const [status, setStatus] = useState<'processing' | 'success' | 'error'>('processing');
  const [message, setMessage] = useState<string>('Processing OAuth callback...');

  useEffect(() => {
    const handleCallback = async () => {
      try {
        // Parse URL parameters
        const urlParams = new URLSearchParams(window.location.search);
        
        // Check for error in callback
        const error = urlParams.get('error');
        const errorDescription = urlParams.get('error_description');
        
        if (error) {
          setStatus('error');
          setMessage(`OAuth error: ${error}${errorDescription ? ` - ${errorDescription}` : ''}`);
          setTimeout(() => {
            window.close(); // Close popup window
          }, 3000);
          return;
        }

        // Get authorization code
        const code = urlParams.get('code');
        if (!code) {
          setStatus('error');
          setMessage('No authorization code received from Microsoft.');
          setTimeout(() => {
            window.close();
          }, 3000);
          return;
        }

        // Exchange code for tokens via backend
        setMessage('Exchanging authorization code for access token...');
        
        // Get orchestrator URL - try gateway first, then orchestrator directly
        const gatewayUrl = localStorage.getItem('root_admin_gateway_url') || 
                          import.meta.env.VITE_GATEWAY_URL || 
                          'http://127.0.0.1:8181';
        const orchestratorUrl = localStorage.getItem('root_admin_orchestrator_url') || 
                               import.meta.env.VITE_ORCHESTRATOR_URL || 
                               gatewayUrl;
        
        const response = await fetch(`${orchestratorUrl}/api/email-teams/exchange-token`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ code }),
        });

        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`Token exchange failed: ${errorText}`);
        }

        const result = await response.json();
        
        if (result.ok && result.access_token) {
          // Set tokens in backend
          await setOAuthTokens(result.access_token, result.refresh_token);
          
          setStatus('success');
          setMessage('OAuth authentication successful! You can now use email and Teams monitoring.');
          
          // Notify parent window if this is a popup
          if (window.opener) {
            window.opener.postMessage({ type: 'oauth_success', access_token: result.access_token }, '*');
          }
          
          setTimeout(() => {
            window.close(); // Close popup window
            // If not a popup, redirect to root (which will show settings)
            if (!window.opener) {
              window.location.href = '/';
            }
          }, 2000);
        } else {
          throw new Error(result.message || 'Token exchange failed');
        }
      } catch (error) {
        console.error('OAuth callback error:', error);
        setStatus('error');
        setMessage(error instanceof Error ? error.message : 'Failed to complete OAuth flow');
        
        // Notify parent window of error
        if (window.opener) {
          window.opener.postMessage({ type: 'oauth_error', error: message }, '*');
        }
        
        setTimeout(() => {
          window.close();
        }, 5000);
      }
    };

    handleCallback();
  }, [message]);

  return (
    <div className="min-h-screen bg-[var(--bg-primary)] flex items-center justify-center p-6">
      <div className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-8 max-w-md w-full text-center">
        <div className="mb-6">
          {status === 'processing' && (
            <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-[var(--bg-steel)] mx-auto"></div>
          )}
          {status === 'success' && (
            <span className="material-symbols-outlined text-4xl text-[var(--success)]">check_circle</span>
          )}
          {status === 'error' && (
            <span className="material-symbols-outlined text-4xl text-[var(--danger)]">error</span>
          )}
        </div>
        <h2 className="text-xl font-bold text-[var(--text-primary)] mb-4">
          {status === 'processing' && 'Processing OAuth Callback'}
          {status === 'success' && 'Authentication Successful'}
          {status === 'error' && 'Authentication Failed'}
        </h2>
        <p className="text-[var(--text-secondary)] text-sm">{message}</p>
        {status === 'processing' && (
          <p className="text-xs text-[rgb(var(--text-secondary-rgb)/0.6)] mt-4">This window will close automatically...</p>
        )}
      </div>
    </div>
  );
};

export default OAuthCallback;
