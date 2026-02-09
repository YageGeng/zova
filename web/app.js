/**
 * PDF Layout Analyzer - Web Interface with WASM
 */

// Import WASM module
import init, { LayoutAnalyzer } from './pkg/zova_wasm.js';

// Initialize WASM
let wasmInitialized = false;
let analyzer = null;

async function initWasm() {
    if (wasmInitialized) return true;
    try {
        await init();
        analyzer = new LayoutAnalyzer();
        wasmInitialized = true;
        console.log('WASM initialized successfully');
        return true;
    } catch (err) {
        console.error('Failed to initialize WASM:', err);
        return false;
    }
}

// DOM Elements
const dropZone = document.getElementById('dropZone');
const fileInput = document.getElementById('fileInput');
const status = document.getElementById('status');
const results = document.getElementById('results');
const pdfCanvas = document.getElementById('pdfCanvas');
const layoutOverlay = document.getElementById('layoutOverlay');
const blockList = document.getElementById('blockList');

// Initialize on page load
initWasm();

// File upload handling
dropZone.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropZone.classList.add('dragover');
});

dropZone.addEventListener('dragleave', () => {
    dropZone.classList.remove('dragover');
});

dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropZone.classList.remove('dragover');

    const files = e.dataTransfer.files;
    if (files.length > 0 && files[0].type === 'application/pdf') {
        processPDF(files[0]);
    }
});

fileInput.addEventListener('change', (e) => {
    if (e.target.files.length > 0) {
        processPDF(e.target.files[0]);
    }
});

// Process PDF file
async function processPDF(file) {
    showStatus('正在加载 PDF...', true);

    // Ensure WASM is initialized
    if (!wasmInitialized) {
        const success = await initWasm();
        if (!success) {
            showStatus('WASM 初始化失败', false);
            return;
        }
    }

    try {
        // Read PDF file
        const arrayBuffer = await file.arrayBuffer();
        const pdfData = new Uint8Array(arrayBuffer);

        // Use pdf.js to render pages (if available)
        if (typeof pdfjsLib !== 'undefined') {
            await processWithPdfJs(pdfData);
        } else {
            // Fallback to stub
            await processStub(pdfData);
        }

    } catch (err) {
        showStatus('处理失败: ' + err.message, false);
        console.error(err);
    }
}

// Process with PDF.js
async function processWithPdfJs(pdfData) {
    const pdf = await pdfjsLib.getDocument({ data: pdfData }).promise;
    const numPages = pdf.numPages;

    showStatus(`PDF 加载成功，共 ${numPages} 页，正在分析...`, true);

    // Process first page as example
    const page = await pdf.getPage(1);
    const viewport = page.getViewport({ scale: 1.0 });

    // Render to canvas
    const canvas = document.createElement('canvas');
    const context = canvas.getContext('2d');
    canvas.width = 1024;
    canvas.height = 1024;

    // Scale to fit 1024x1024
    const scale = Math.min(1024 / viewport.width, 1024 / viewport.height);
    const scaledViewport = page.getViewport({ scale });

    await page.render({
        canvasContext: context,
        viewport: scaledViewport
    }).promise;

    // Get image data
    const imageData = context.getImageData(0, 0, 1024, 1024);
    const floatData = new Float32Array(3 * 1024 * 1024);

    // Convert to CHW format and normalize
    for (let i = 0; i < 1024 * 1024; i++) {
        const r = imageData.data[i * 4] / 255.0;
        const g = imageData.data[i * 4 + 1] / 255.0;
        const b = imageData.data[i * 4 + 2] / 255.0;

        floatData[i] = r;
        floatData[i + 1024 * 1024] = g;
        floatData[i + 2 * 1024 * 1024] = b;
    }

    // Run analysis
    const result = analyzer.analyze(Array.from(floatData), 1024, 1024);

    showStatus('分析完成！', false);
    displayResults(pdfData, result, canvas);
}

// Stub processing
async function processStub(pdfData) {
    await new Promise(resolve => setTimeout(resolve, 1000));

    showStatus('分析完成！', false);
    displayResults(pdfData, getStubResult(), null);
}

// Show status message
function showStatus(message, loading) {
    const spinner = loading ? '<span class="spinner"></span>' : '';
    status.innerHTML = spinner + message;
}

// Stub result for testing
function getStubResult() {
    return {
        pages: [{
            page_num: 0,
            width: 595,
            height: 842,
            blocks: [
                {
                    id: "p0-b0",
                    bbox: [50, 50, 545, 100],
                    class: "Title",
                    text: "Sample Title"
                },
                {
                    id: "p0-b1",
                    bbox: [50, 120, 545, 300],
                    class: "Text",
                    text: "This is a sample paragraph block."
                },
                {
                    id: "p0-b2",
                    bbox: [50, 320, 300, 500],
                    class: "Image",
                    text: null
                }
            ]
        }]
    };
}

// Display analysis results
function displayResults(pdfData, result, sourceCanvas) {
    results.classList.add('active');

    // Render first page
    const page = result.pages[0];
    renderPage(page, sourceCanvas);

    // Render layout blocks
    renderLayoutBlocks(page);

    // Render block list
    renderBlockList(page);
}

// Render page canvas
function renderPage(page, sourceCanvas) {
    const ctx = pdfCanvas.getContext('2d');
    const scale = 800 / page.width;

    pdfCanvas.width = 800;
    pdfCanvas.height = page.height * scale;

    if (sourceCanvas) {
        // Draw from source canvas
        ctx.drawImage(sourceCanvas, 0, 0, pdfCanvas.width, pdfCanvas.height);
    } else {
        // Clear canvas
        ctx.fillStyle = 'white';
        ctx.fillRect(0, 0, pdfCanvas.width, pdfCanvas.height);

        // Draw placeholder text
        ctx.fillStyle = '#999';
        ctx.font = '16px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(
            'PDF Preview (PDF.js not loaded)',
            pdfCanvas.width / 2,
            pdfCanvas.height / 2
        );

        // Draw page border
        ctx.strokeStyle = '#ddd';
        ctx.strokeRect(0, 0, pdfCanvas.width, pdfCanvas.height);
    }
}

// Render layout overlay
function renderLayoutBlocks(page) {
    layoutOverlay.innerHTML = '';
    layoutOverlay.style.width = pdfCanvas.width + 'px';
    layoutOverlay.style.height = pdfCanvas.height + 'px';

    const scale = 800 / page.width;

    page.blocks.forEach(block => {
        const [x, y, x2, y2] = block.bbox;
        const width = (x2 - x) * scale;
        const height = (y2 - y) * scale;

        const box = document.createElement('div');
        box.className = `layout-box ${block.class.toLowerCase()}`;
        box.style.left = `${x * scale}px`;
        box.style.top = `${y * scale}px`;
        box.style.width = `${width}px`;
        box.style.height = `${height}px`;
        box.title = `${block.class}: ${block.id}`;

        box.addEventListener('click', () => {
            highlightBlock(block.id);
        });

        layoutOverlay.appendChild(box);
    });
}

// Render block list
function renderBlockList(page) {
    blockList.innerHTML = '';

    page.blocks.forEach(block => {
        const item = document.createElement('div');
        item.className = `block-item ${block.class.toLowerCase()}`;
        item.innerHTML = `
            <strong>${block.class}</strong>
            <br>
            <small>${block.id}</small>
            ${block.text ? `<br><small>${block.text.substring(0, 50)}...</small>` : ''}
        `;

        item.addEventListener('click', () => {
            highlightBlock(block.id);
        });

        blockList.appendChild(item);
    });
}

// Highlight a block
function highlightBlock(blockId) {
    // Remove previous highlights
    document.querySelectorAll('.layout-box').forEach(box => {
        box.style.opacity = '0.3';
    });
    document.querySelectorAll('.block-item').forEach(item => {
        item.style.background = 'white';
    });

    // Highlight selected
    const box = document.querySelector(`.layout-box[title*="${blockId}"]`);
    if (box) {
        box.style.opacity = '1';
        box.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }

    const items = document.querySelectorAll('.block-item');
    items.forEach(item => {
        if (item.innerHTML.includes(blockId)) {
            item.style.background = '#e3f2fd';
            item.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
    });
}

// Export to JSON
window.exportJSON = function() {
    const result = getStubResult();
    const json = JSON.stringify(result, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = url;
    a.download = 'layout-analysis.json';
    a.click();

    URL.revokeObjectURL(url);
};