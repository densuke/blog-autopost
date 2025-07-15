from tweepy import Client
from . import SocialMediaPlugin

class X(SocialMediaPlugin):
    def __init__(self, consumer_key, consumer_secret, access_token, access_token_secret):
        self.client = Client(
            consumer_key=consumer_key,
            consumer_secret=consumer_secret,
            access_token=access_token,
            access_token_secret=access_token_secret
        )

    def post(self, title: str, link: str):
        tweet_text = f"{title} {link}"
        response = self.client.create_tweet(text=tweet_text)
        print(f"Xに投稿しました: {response.data['id']}")
