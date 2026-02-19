"""
Simple Flask application for Lambda.

This app demonstrates a typical Flask API that can run on RustStack Lambda.
"""

import json
import os
from flask import Flask, jsonify, request

app = Flask(__name__)

# In-memory storage (in production, use S3/DynamoDB)
items = {}

@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({
        'status': 'healthy',
        'service': 'flask-lambda',
        'region': os.environ.get('AWS_REGION', 'unknown')
    })

@app.route('/items', methods=['GET'])
def list_items():
    """List all items."""
    return jsonify({'items': list(items.values())})

@app.route('/items/<item_id>', methods=['GET'])
def get_item(item_id):
    """Get a single item by ID."""
    if item_id not in items:
        return jsonify({'error': 'Item not found'}), 404
    return jsonify(items[item_id])

@app.route('/items', methods=['POST'])
def create_item():
    """Create a new item."""
    data = request.get_json()
    if not data or 'name' not in data:
        return jsonify({'error': 'Name required'}), 400

    import uuid
    item_id = str(uuid.uuid4())
    item = {
        'id': item_id,
        'name': data['name'],
        'description': data.get('description', '')
    }
    items[item_id] = item
    return jsonify(item), 201

@app.route('/items/<item_id>', methods=['PUT'])
def update_item(item_id):
    """Update an existing item."""
    if item_id not in items:
        return jsonify({'error': 'Item not found'}), 404

    data = request.get_json()
    if data.get('name'):
        items[item_id]['name'] = data['name']
    if data.get('description'):
        items[item_id]['description'] = data['description']

    return jsonify(items[item_id])

@app.route('/items/<item_id>', methods=['DELETE'])
def delete_item(item_id):
    """Delete an item."""
    if item_id not in items:
        return jsonify({'error': 'Item not found'}), 404

    del items[item_id]
    return '', 204


# Lambda handler using Mangum (WSGI adapter for Lambda)
try:
    from mangum import Mangum
    handler = Mangum(app)
except ImportError:
    # Fallback for direct testing without Mangum
    def handler(event, context):
        """Simple handler for testing without Mangum."""
        return {
            'statusCode': 200,
            'body': json.dumps({'message': 'Mangum not installed, running fallback'})
        }


if __name__ == '__main__':
    app.run(debug=True)
