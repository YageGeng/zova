/**
 * PDF Layout Analyzer - Web Interface
 * 
 * This module handles:
 * - PDF file upload and preview
 * - WASM module initialization
 * - Layout detection visualization
 * - Text extraction display
 */

// WASM module instance
let wasmModule = null;

// Initialize WASM
async function initWasm() {
    try {
        // TODO: Load actual WASM module
        // wasmModule = await import('./pkg/zova_wasm.js');
        // await wasmModule.default();
        
        console.log('WASM initialized (stub)');
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
    
    try {
        const arrayBuffer = await file.arrayBuffer();
        const uint8Array = new Uint8Array(arrayBuffer);
        
        // TODO: Call WASM to process PDF
        // const result = await wasmModule.process_pdf(uint8Array);
        
        // For now, show stub result
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        showStatus('分析完成！', false);
        displayResults(uint8Array, getStubResult());
        
    } catch (err) {
        showStatus('处理失败: ' + err.message, false);
        console.error(err);
    }
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
                    content: { text: "Sample Title" }
                },
                {
                    id: "p0-b1",
                    bbox: [50, 120, 545, 300],
                    class: "Paragraph",
                    content: { text: "This is a sample paragraph block." }
                },
                {
                    id: "p0-b2",
                    bbox: [50, 320, 300, 500],
                    class: "Image",
                    content: {}
                }
            ]
        }]
    };
}

// Display analysis results
function displayResults(pdfData, result) {
    results.classList.add('active');
    
    // Render first page
    const page = result.pages[0];
    renderPage(page);
    
    // Render layout blocks
    renderLayoutBlocks(page);
    
    // Render block list
    renderBlockList(page);
}

// Render page canvas
function renderPage(page) {
    const ctx = pdfCanvas.getContext('2d');
    const scale = 800 / page.width;
    
    pdfCanvas.width = 800;
    pdfCanvas.height = page.height * scale;
    
    // Clear canvas
    ctx.fillStyle = 'white';
    ctx.fillRect(0, 0, pdfCanvas.width, pdfCanvas.height);
    
    // Draw placeholder text
    ctx.fillStyle = '#999';
    ctx.font = '16px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(
        'PDF Preview (WASM rendering not yet implemented)',
        pdfCanvas.width / 2,
        pdfCanvas.height / 2
    );
}

// Render layout overlay
function renderLayoutBlocks(page) {
    layoutOverlay.innerHTML = '';
    
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
function exportJSON() {
    const result = getStubResult();
    const json = JSON.stringify(result, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    
    const a = document.createElement('a');
    a.href = url;
    a.download = 'layout-analysis.json';
    a.click();
    
    URL.revokeObjectURL(url);
}

// Initialize
initWasm().then(success => {
    if (success) {
        console.log('PDF Layout Analyzer ready');
    }
});