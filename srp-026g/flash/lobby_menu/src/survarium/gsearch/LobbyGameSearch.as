package survarium.gsearch
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.filters.DropShadowFilter;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import flash.utils.Timer;
   import scaleform.clik.events.ButtonEvent;
   import survarium.Util;
   import survarium.data.LocalizationData;
   
   public class LobbyGameSearch extends MovieClip
   {
      
      public var textTime:TextField;
      
      public var textSearchLabel:TextField;
      
      public var rangeLabel:TextField;
      
      public var matchSearchProgress:MovieClip;
      
      public var progressIndicator:MovieClip;
      
      public var rangeGate:MovieClip;
      
      public var arrowLine:MovieClip;
      
      public var columnsContainer:MovieClip = new MovieClip();
      
      protected var label_sync:TextField = new TextField();
      
      protected var label_sync_fr:TextFormat;
      
      private var _dots:String = "";
      
      private var _startTime:int = 0;
      
      protected var columnCount:uint = 20;
      
      protected var columnRenderers:Array = new Array();
      
      protected var maxColumnValue:uint = 100;
      
      protected var maxColumnHeight:uint = 17;
      
      protected const MIN_VALUE:uint = 30;
      
      protected var _timer:Timer = new Timer(1000);
      
      public function LobbyGameSearch()
      {
         super();
         stop();
         this.y = y + 1;
         textTime = this.matchSearchProgress.labelText.textField;
         textSearchLabel = this.matchSearchProgress.labelSearchText.textField;
         rangeLabel = this.matchSearchProgress.textFieldRange;
         progressIndicator = this.matchSearchProgress.progressIndicator;
         rangeGate = this.matchSearchProgress.rangeGate;
         arrowLine = this.matchSearchProgress.whiteLine;
         this.textSearchLabel.y += 2;
         this.arrowLine.y += 3;
         var _loc1_:TextField = this.matchSearchProgress.textFieldHint;
         _loc1_.visible = false;
         this.matchSearchProgress.addChild(this.columnsContainer);
         this.textTime.width += 55;
         var _loc2_:TextFormat = this.textTime.getTextFormat();
         _loc2_.align = TextFormatAlign.CENTER;
         this.label_sync.width = 216;
         this.label_sync.height = 105;
         this.label_sync.multiline = true;
         this.label_sync.wordWrap = true;
         this.label_sync.selectable = false;
         label_sync_fr = new TextFormat("calibri",16,16777215,true);
         this.label_sync_fr.leading = -3;
         this.label_sync_fr.align = TextFormatAlign.CENTER;
         this.label_sync.defaultTextFormat = this.label_sync_fr;
         this.label_sync.filters = [new DropShadowFilter(0,0,0,1,9,9,1.5)];
         this._timer.addEventListener(TimerEvent.TIMER,this.onTimerUpdate);
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1);
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1);
      }
      
      private function onTimerUpdate(param1:TimerEvent) : void
      {
         var _loc2_:String = Util.getTimeString(this._startTime + this._timer.currentCount);
         currentTime = _loc2_;
         this.updateDots();
      }
      
      private function updateDots() : void
      {
         _dots += ".";
         this.textSearchLabel.text = LocalizationData.getName("search_label") + this._dots;
         if(this._dots.length == 3)
         {
            _dots = "";
         }
      }
      
      public function updateTime(param1:int) : void
      {
         _startTime = param1;
         this._timer.reset();
         this._timer.start();
         var _loc2_:String = Util.getTimeString(this._startTime);
         currentTime = _loc2_;
      }
      
      public function setSearchRange(param1:uint, param2:uint) : void
      {
         var _loc3_:Sprite = this.matchSearchProgress.whiteLine;
         var _loc4_:Sprite = this.matchSearchProgress.rangeGate;
         var _loc5_:uint = this.columnRenderers.length;
         param2 = Math.min(_loc5_ - 1,param2);
         var _loc6_:int = param2 - param1;
         var _loc7_:Number = (_loc3_.width - 5) / (this.columnCount - 1);
         if(_loc6_ == 0)
         {
            _loc4_.x = int(_loc3_.x + param1 * _loc7_ + 2);
            _loc4_.width = 1;
            this.rangeLabel.text = LocalizationData.getName("search_range") + ":" + param1.toString();
         }
         else
         {
            _loc4_.x = int(_loc3_.x + param1 * _loc7_ + 2);
            _loc4_.width = _loc6_ * _loc7_ + 1;
            this.rangeLabel.text = LocalizationData.getName("search_range") + ":" + param1.toString() + "–" + param2.toString();
            trace("rangeGate::",_loc4_.width,_loc6_,_loc7_);
         }
         var _loc8_:int = 0;
         var _loc9_:MovieClip;
         var _loc10_:int;
         var _loc11_:int;
         var _loc12_:ColorTransform;
         while(_loc8_ < _loc5_)
         {
            _loc9_ = this.columnRenderers[_loc8_];
            _loc10_ = _loc8_ == 0 ? 0 : (this.columnRenderers[_loc8_ - 1].level + 1);
            _loc11_ = (_loc9_.level);
            if(_loc10_ < param1 || _loc11_ > param2)
            {
               _loc9_.transform.colorTransform = new ColorTransform();
               _loc9_.alpha = 0.15;
            }
            else
            {
               _loc12_ = new ColorTransform();
               _loc12_.color = 16758295;
               _loc9_.transform.colorTransform = _loc12_;
               _loc9_.alpha = 1;
            }
            _loc8_++;
         }
      }
      
      public function init() : void
      {
         if(!visible)
         {
            enabled = true;
            visible = true;
            _startTime = 0;
            this._timer.reset();
            this._timer.start();
            this.playIndicator();
            this.onTimerUpdate(null);
         }
      }
      
      private function playIndicator() : void
      {
         trace("playIndicator",this.progressIndicator.currentFrame);
         if(this.progressIndicator.currentFrame == 1)
         {
            this.progressIndicator.gotoAndPlay(2);
         }
      }
      
      public function close() : void
      {
         enabled = false;
         visible = false;
         this._timer.stop();
      }
      
      public function setSearchProperties(param1:Array, param2:Array = null) : void
      {
         var _loc3_:uint = param1.length;
         columnCount = _loc3_;
         var _loc4_:Sprite = this.matchSearchProgress.whiteLine;
         var _loc5_:Number = (_loc4_.width - 5) / (this.columnCount - 1);
         trace("setSearchProperties::",_loc3_,this.columnCount,_loc4_.width,_loc5_);
         maxColumnValue = this.MIN_VALUE;
         var _loc6_:int = 0;
         var _loc7_:MovieClip;
         var _loc8_:int;
         var _loc9_:int;
         while(_loc6_ < _loc3_)
         {
            maxColumnValue = Math.max(this.maxColumnValue,param1[_loc6_].value);
            _loc6_++;
         }
         _loc6_ = 0;
         while(_loc6_ < _loc3_)
         {
            if(!this.columnRenderers[_loc6_])
            {
               _loc7_ = new ColumnSearchProgress();
               _loc7_.addEventListener(MouseEvent.ROLL_OVER,this.updateCountHint);
               _loc7_.addEventListener(MouseEvent.ROLL_OUT,this.updateCountHint);
               _loc7_.transform.colorTransform = new ColorTransform();
               _loc7_.alpha = 0.15;
               this.columnRenderers.push(_loc7_);
            }
            else
            {
               _loc7_ = this.columnRenderers[_loc6_];
            }
            _loc7_.y = _loc4_.y - 6;
            _loc7_.x = int(_loc4_.x + _loc5_ * _loc6_);
            _loc7_.mc_back.width = _loc5_;
            _loc7_.mc_back.height = this.maxColumnHeight;
            _loc7_.mc_back.x = (5 - _loc5_) / 2;
            _loc8_ = (param1[_loc6_].value);
            _loc9_ = (param1[_loc6_].level);
            trace("setSearchProperties::",_loc6_,_loc8_,_loc9_,_loc7_.x,_loc7_.mc_column.width,_loc5_,_loc4_.x,x);
            _loc7_.mc_column.height = this.maxColumnHeight * (_loc8_ / this.maxColumnValue);
            _loc7_.value = _loc8_;
            _loc7_.level = _loc9_;
            this.columnsContainer.addChild(_loc7_);
            _loc6_++;
         }
      }
      
      private function updateCountHint(param1:MouseEvent) : void
      {
         var _loc2_:MovieClip = param1.target as MovieClip;
         var _loc3_:TextField = this.matchSearchProgress.textFieldHint;
         if(param1.type == MouseEvent.ROLL_OVER)
         {
            _loc3_.visible = true;
            _loc3_.text = String(_loc2_.value);
            _loc3_.textColor = _loc2_.alpha == 1 ? 16758295 : 7236716;
            _loc3_.x = _loc2_.x - _loc3_.textWidth / 2 + 3;
            _loc3_.y = _loc2_.y - _loc2_.height - 18;
         }
         else
         {
            _loc3_.visible = false;
         }
      }
      
      public function set sync_text(param1:String) : void
      {
         trace("sync_text");
         if(!this.contains(this.label_sync) || this.label_sync.text != param1)
         {
            addChild(this.label_sync);
            this.label_sync.x = this.matchSearchProgress.x + 4;
            this.textTime.visible = false;
            this.textSearchLabel.visible = false;
            this.rangeLabel.visible = false;
            this.progressIndicator.visible = false;
            this.progressIndicator.gotoAndStop(1);
            this.rangeGate.visible = false;
            this.arrowLine.visible = false;
            this.columnsContainer.visible = false;
            this.label_sync.text = param1;
            this.label_sync.y = (74 - this.label_sync.textHeight) / 2 + 6;
         }
      }
      
      public function set currentTime(param1:String) : void
      {
         this.textTime.text = param1;
      }
      
      public function set averageTime(param1:String) : void
      {
      }
      
      public function reset_sync() : void
      {
         trace("reset_sync");
         if(this.contains(this.label_sync))
         {
            removeChild(this.label_sync);
            this.textTime.visible = true;
            this.textSearchLabel.visible = true;
            this.rangeLabel.visible = true;
            this.playIndicator();
            this.progressIndicator.visible = true;
            this.rangeGate.visible = true;
            this.arrowLine.visible = true;
            this.columnsContainer.visible = true;
         }
      }
      
      public function setPlace(param1:uint, param2:uint) : void
      {
      }
      
      protected function leaveQuenue(param1:ButtonEvent) : void
      {
         ExternalInterface.call("leave_queue");
      }
   }
}

