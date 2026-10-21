package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.constants.Colors;
   
   public class RankingItemRenderer extends ListItemRenderer
   {
      
      public var moveSing:MovieClip;
      
      public var txPlace:TextField;
      
      public var txNickname:TextField;
      
      public var txRanking:TextField;
      
      public var txRankingMoved:TextField;
      
      public var background:MovieClip;
      
      public function RankingItemRenderer()
      {
         super();
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(param1:Object) : void
      {
         var _loc2_:Boolean;
         if(_data == param1)
         {
            return;
         }
         if(name == "playerPlace")
         {
            this.background.visible = false;
         }
         if(this.background)
         {
            this.background.gotoAndStop(index % 2 == 0 ? 1 : 2);
         }
         _data = param1;
         if(_data)
         {
            this.txPlace.text = _data.place;
            this.txNickname.text = _data.nickname;
            this.txRanking.text = _data.ranking;
            if(name != "playerPlace")
            {
               if(_data.is_local)
               {
                  this.txNickname.textColor = Colors.PLAYER;
                  this.txPlace.textColor = Colors.PLAYER;
                  this.txRanking.textColor = Colors.PLAYER;
               }
               else if(this.txNickname.textColor != 13354941)
               {
                  this.txNickname.textColor = 13354941;
                  this.txPlace.textColor = 13354941;
                  this.txRanking.textColor = 13354941;
               }
            }
            if(_data.ranking_diff != undefined && _data.ranking_diff != 0)
            {
               _loc2_ = _data.ranking_diff > 0;
               this.txRankingMoved.text = (_loc2_ ? "(+" : "(") + String(_data.ranking_diff) + ")";
               this.txRankingMoved.textColor = !_loc2_ ? 16743007 : 16777215;
               if(name != "playerPlace")
               {
                  this.txRankingMoved.x = int(this.txRanking.x + this.txRanking.textWidth) + 3;
               }
               else
               {
                  this.txRanking.x = int(this.txRankingMoved.x + this.txRankingMoved.getLineMetrics(0).x - this.txRanking.textWidth - this.txRanking.getLineMetrics(0).x) - 3;
               }
               this.moveSing.gotoAndStop(_loc2_ ? "up" : "down");
            }
            else
            {
               this.txRankingMoved.text = "";
               this.moveSing.gotoAndStop("empty");
            }
         }
         else
         {
            this.txPlace.text = "";
            this.txNickname.text = "";
            this.txRanking.text = "";
            this.txRankingMoved.text = "";
            this.moveSing.gotoAndStop("empty");
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.moveSing.gotoAndStop("empty");
         preventAutosizing = true;
      }
   }
}

