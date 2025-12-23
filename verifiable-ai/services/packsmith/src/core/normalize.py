from markdownify import markdownify as md
from bs4 import BeautifulSoup

def normalize_html(html_content: str) -> str:
    """
    Normalizes HTML to Markdown.
    strips scripts, styles, nav, footer.
    """
    soup = BeautifulSoup(html_content, 'html.parser')
    
    # Remove unwanted tags
    for tag in soup(['script', 'style', 'nav', 'footer', 'iframe', 'noscript']):
        tag.decompose()
        
    # Convert to markdown
    text = md(str(soup), heading_style="ATX")
    return text.strip()

def normalize_text(text: str) -> str:
    # Basic cleanup
    return text.strip()
