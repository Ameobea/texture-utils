from flask import Flask, request
from flask_cors import CORS
import numpy as np
import umap

app = Flask(__name__)
CORS(app)

def embed_colors(colors: list[list[int]], n_neighbors: int = 25) -> list[float]:
    data = np.array(colors)

    fit = umap.UMAP(n_neighbors=n_neighbors, n_components=1)
    projected = fit.fit_transform(data)


    min = np.min(projected)
    max = np.max(projected)
    projected = [float(x[0]) for x in projected]

    # Scale to 0-1
    projected = [(x - min) / (max - min) for x in projected]
    return projected

@app.route('/embed', methods=['POST'])
def embed():
    colors = request.get_json()

    n_neighbors = request.args.get('n_neighbors', default=25, type=int)

    if not isinstance(colors, list):
        return 'Colors must be a list', 400
    if not all(isinstance(color, list) for color in colors):
        return 'Colors must be a list of lists', 400
    if not all(len(color) == 3 for color in colors):
        return 'Colors must be a list of lists of length 3', 400
    if not all(all(isinstance(c, int) for c in color) for color in colors):
        return 'Colors must be a list of lists of integers', 400
    if not all(all(0 <= c <= 255 for c in color) for color in colors):
        return 'Colors must be a list of lists of integers between 0 and 255', 400

    projected = embed_colors(colors, n_neighbors)
    return projected
