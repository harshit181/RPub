
const browserAPI = typeof browser !== 'undefined' ? browser : chrome;

document.addEventListener('DOMContentLoaded', async () => {
    const sendBtn = document.getElementById('send-btn');
    const messageEl = document.getElementById('message');
    const currentUrlEl = document.getElementById('current-url');
    const configuredHostEl = document.getElementById('configured-host');


    const config = await browserAPI.storage.local.get(['endpoint', 'username', 'password']);
    if (config.endpoint) {
        try {
            const url = new URL(config.endpoint);
            configuredHostEl.textContent = url.host;
            configuredHostEl.title = config.endpoint;
        } catch {
            configuredHostEl.textContent = config.endpoint;
        }
    } else {
        configuredHostEl.textContent = 'Not configured';
    }


    let currentUrl = '';
    try {
        const tabs = await browserAPI.tabs.query({ active: true, currentWindow: true });
        if (tabs[0]) {
            currentUrl = tabs[0].url;
            currentUrlEl.textContent = currentUrl.length > 50
                ? currentUrl.substring(0, 50) + '...'
                : currentUrl;
            currentUrlEl.title = currentUrl;
        }
    } catch (error) {
        currentUrlEl.textContent = 'Unable to get URL';
        console.error('Error getting current tab:', error);
    }


    sendBtn.addEventListener('click', async () => {
        messageEl.textContent = '';
        messageEl.className = 'message';


        const config = await browserAPI.storage.local.get(['endpoint', 'username', 'password']);

        if (!config.endpoint) {
            messageEl.textContent = 'Please configure the RssPub host URL in settings first.';
            messageEl.className = 'message error';
            return;
        }

        if (!currentUrl) {
            messageEl.textContent = 'No URL to send.';
            messageEl.className = 'message error';
            return;
        }

        sendBtn.disabled = true;
        sendBtn.textContent = 'Sending...';

        try {
            const headers = {
                'Content-Type': 'application/json'
            };


            if (config.username && config.password) {
                const credentials = btoa(`${config.username}:${config.password}`);
                headers['Authorization'] = `Basic ${credentials}`;
            }


            const baseUrl = config.endpoint.replace(/\/$/, '');
            const fullUrl = `${baseUrl}/read-it-later`;

            const response = await fetch(fullUrl, {
                method: 'POST',
                headers: headers,
                body: JSON.stringify({ url: currentUrl })
            });

            if (response.ok) {
                messageEl.textContent = 'URL sent successfully!';
                messageEl.className = 'message success';
            } else {
                messageEl.textContent = `Error: ${response.status} ${response.statusText}`;
                messageEl.className = 'message error';
            }
        } catch (error) {
            messageEl.textContent = `Failed to send: ${error.message}`;
            messageEl.className = 'message error';
        } finally {
            sendBtn.disabled = false;
            sendBtn.textContent = 'Send URL';
        }
    });
});
