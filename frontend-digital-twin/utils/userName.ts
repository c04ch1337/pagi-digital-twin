/**
 * Utility functions for managing user name
 */

/**
 * Gets the user's display name from localStorage
 * Returns "ROOT ADMIN" if no name is set
 */
export function getUserName(): string {
  const userName = localStorage.getItem('root_admin_user_name');
  return userName?.trim() || 'ROOT ADMIN';
}

/**
 * Sets the user's name in localStorage
 */
export function setUserName(name: string): void {
  if (name.trim()) {
    localStorage.setItem('root_admin_user_name', name.trim());
  } else {
    localStorage.removeItem('root_admin_user_name');
  }
}
