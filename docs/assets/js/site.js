// Deep Cuts Website Interactivity

document.addEventListener('DOMContentLoaded', () => {
  // 1. Dynamic Card Glow Effect
  const cards = document.querySelectorAll('.feature-card');
  
  cards.forEach(card => {
    card.addEventListener('mousemove', (e) => {
      const rect = card.getBoundingClientRect();
      const x = e.clientX - rect.left; // x position within the element
      const y = e.clientY - rect.top;  // y position within the element
      
      // Update custom properties on hover to animate gradients dynamically
      card.style.setProperty('--mouse-x', `${x}px`);
      card.style.setProperty('--mouse-y', `${y}px`);
    });
  });

  // 2. Scroll Active Header Effect
  const header = document.querySelector('.site-header');
  window.addEventListener('scroll', () => {
    if (window.scrollY > 20) {
      header.style.background = 'rgba(10, 11, 16, 0.9)';
      header.style.padding = '0.75rem 0';
    } else {
      header.style.background = 'rgba(10, 11, 16, 0.75)';
      header.style.padding = '1rem 0';
    }
  });
});
