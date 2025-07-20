from tweepy import Client
from . import SocialMediaPlugin

class X(SocialMediaPlugin):
    def __init__(self, consumer_key, consumer_secret, access_token, access_token_secret):
        self.sns_type = "x"
        self.client = Client(
            consumer_key=consumer_key,
            consumer_secret=consumer_secret,
            access_token=access_token,
            access_token_secret=access_token_secret
        )

    def post(self, optimized_text: str):
        response = self.client.create_tweet(text=optimized_text)
        print(f"Xに投稿しました: {response.data['id']}")
