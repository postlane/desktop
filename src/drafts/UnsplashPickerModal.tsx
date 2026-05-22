// SPDX-License-Identifier: BUSL-1.1

export interface UnsplashUrls {
  raw: string; full: string; regular: string; small: string; thumb: string;
}
export interface UnsplashLinks { download_location: string; }
export interface UnsplashUserLinks { html: string; }
export interface UnsplashUser { name: string; links: UnsplashUserLinks; }
export interface UnsplashPhoto {
  id: string;
  description: string | null;
  urls: UnsplashUrls;
  links: UnsplashLinks;
  user: UnsplashUser;
}
export interface Attribution {
  photographer_name: string;
  photographer_url: string;
}

interface Props {
  photos: UnsplashPhoto[];
  onSelect: (_url: string, _dl: string, _attr: Attribution) => void;
  onClose: () => void;
  onLoadMore: () => void;
  loadingMore: boolean;
}

export default function UnsplashPickerModal({ photos, onSelect, onClose, onLoadMore, loadingMore }: Props) {
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" style={{ width: '720px', maxWidth: '95vw' }}>
        <header className="modal-card-head" style={{ padding: '0.75rem 1rem' }}>
          <p className="modal-card-title is-size-6">Select a photo</p>
          <button className="delete" aria-label="Close" onClick={onClose} />
        </header>
        <section className="modal-card-body" style={{ overflowY: 'scroll', maxHeight: '60vh', padding: '0.75rem' }}>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '0.5rem' }}>
            {photos.map((photo) => (
              <button key={photo.id} type="button"
                onClick={() => onSelect(photo.urls.regular, photo.links.download_location, {
                  photographer_name: photo.user.name,
                  photographer_url: photo.user.links.html,
                })}
                style={{ padding: 0, border: 'none', background: 'none', cursor: 'pointer', display: 'block' }}>
                <img
                  src={photo.urls.thumb}
                  alt={photo.description ?? 'Unsplash photo'}
                  style={{ width: '100%', aspectRatio: '1', objectFit: 'cover', borderRadius: '0.25rem', display: 'block' }}
                />
              </button>
            ))}
          </div>
        </section>
        <footer className="modal-card-foot" style={{ padding: '0.75rem 1rem', gap: '0.5rem' }}>
          <button className="button is-small" onClick={onLoadMore} disabled={loadingMore}
            aria-label="Load more">
            {loadingMore ? 'Loading…' : 'Load more'}
          </button>
          <button className="button is-small is-light" onClick={onClose} aria-label="Cancel">
            Cancel
          </button>
        </footer>
      </div>
    </div>
  );
}
