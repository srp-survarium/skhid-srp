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
         var hintField:TextField = this.matchSearchProgress.textFieldHint;
         hintField.visible = false;
         this.matchSearchProgress.addChild(this.columnsContainer);
         this.textTime.width += 55;
         var timeFormat:TextFormat = this.textTime.getTextFormat();
         timeFormat.align = TextFormatAlign.CENTER;
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
      
      override public function set x(value:Number) : void
      {
         super.x = int(value);
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value);
      }
      
      private function onTimerUpdate(event:TimerEvent) : void
      {
         var timeString:String = Util.getTimeString(this._startTime + this._timer.currentCount);
         currentTime = timeString;
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
      
      public function updateTime(startTime:int) : void
      {
         _startTime = startTime;
         this._timer.reset();
         this._timer.start();
         var timeString:String = Util.getTimeString(this._startTime);
         currentTime = timeString;
      }
      
      public function setSearchRange(minLevel:uint, maxLevel:uint) : void
      {
         var line:Sprite = this.matchSearchProgress.whiteLine;
         var gate:Sprite = this.matchSearchProgress.rangeGate;
         var columnTotal:uint = this.columnRenderers.length;
         maxLevel = Math.min(columnTotal - 1,maxLevel);
         var span:int = maxLevel - minLevel;
         var step:Number = (line.width - 5) / (this.columnCount - 1);
         if(span == 0)
         {
            gate.x = int(line.x + minLevel * step + 2);
            gate.width = 1;
            this.rangeLabel.text = LocalizationData.getName("search_range") + ":" + minLevel.toString();
         }
         else
         {
            gate.x = int(line.x + minLevel * step + 2);
            gate.width = span * step + 1;
            this.rangeLabel.text = LocalizationData.getName("search_range") + ":" + minLevel.toString() + "–" + maxLevel.toString();
            trace("rangeGate::",gate.width,span,step);
         }
         var i:int = 0;
         var column:MovieClip;
         var fromLevel:int;
         var toLevel:int;
         var highlight:ColorTransform;
         while(i < columnTotal)
         {
            column = this.columnRenderers[i];
            fromLevel = i == 0 ? 0 : (this.columnRenderers[i - 1].level + 1);
            toLevel = (column.level);
            if(fromLevel < minLevel || toLevel > maxLevel)
            {
               column.transform.colorTransform = new ColorTransform();
               column.alpha = 0.15;
            }
            else
            {
               highlight = new ColorTransform();
               highlight.color = 16758295;
               column.transform.colorTransform = highlight;
               column.alpha = 1;
            }
            i++;
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
      
      public function setSearchProperties(columns:Array, extra:Array = null) : void
      {
         var columnTotal:uint = columns.length;
         columnCount = columnTotal;
         var line:Sprite = this.matchSearchProgress.whiteLine;
         var step:Number = (line.width - 5) / (this.columnCount - 1);
         trace("setSearchProperties::",columnTotal,this.columnCount,line.width,step);
         maxColumnValue = this.MIN_VALUE;
         var i:int = 0;
         var column:MovieClip;
         var columnValue:int;
         var columnLevel:int;
         while(i < columnTotal)
         {
            maxColumnValue = Math.max(this.maxColumnValue,columns[i].value);
            i++;
         }
         i = 0;
         while(i < columnTotal)
         {
            if(!this.columnRenderers[i])
            {
               column = new ColumnSearchProgress();
               column.addEventListener(MouseEvent.ROLL_OVER,this.updateCountHint);
               column.addEventListener(MouseEvent.ROLL_OUT,this.updateCountHint);
               column.transform.colorTransform = new ColorTransform();
               column.alpha = 0.15;
               this.columnRenderers.push(column);
            }
            else
            {
               column = this.columnRenderers[i];
            }
            column.y = line.y - 6;
            column.x = int(line.x + step * i);
            column.mc_back.width = step;
            column.mc_back.height = this.maxColumnHeight;
            column.mc_back.x = (5 - step) / 2;
            columnValue = (columns[i].value);
            columnLevel = (columns[i].level);
            trace("setSearchProperties::",i,columnValue,columnLevel,column.x,column.mc_column.width,step,line.x,x);
            column.mc_column.height = this.maxColumnHeight * (columnValue / this.maxColumnValue);
            column.value = columnValue;
            column.level = columnLevel;
            this.columnsContainer.addChild(column);
            i++;
         }
      }
      
      private function updateCountHint(event:MouseEvent) : void
      {
         var column:MovieClip = event.target as MovieClip;
         var hintField:TextField = this.matchSearchProgress.textFieldHint;
         if(event.type == MouseEvent.ROLL_OVER)
         {
            hintField.visible = true;
            hintField.text = String(column.value);
            hintField.textColor = column.alpha == 1 ? 16758295 : 7236716;
            hintField.x = column.x - hintField.textWidth / 2 + 3;
            hintField.y = column.y - column.height - 18;
         }
         else
         {
            hintField.visible = false;
         }
      }
      
      public function set sync_text(value:String) : void
      {
         trace("sync_text");
         if(!this.contains(this.label_sync) || this.label_sync.text != value)
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
            this.label_sync.text = value;
            this.label_sync.y = (74 - this.label_sync.textHeight) / 2 + 6;
         }
      }
      
      public function set currentTime(value:String) : void
      {
         this.textTime.text = value;
      }
      
      public function set averageTime(value:String) : void
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
      
      public function setPlace(place:uint, queueSize:uint) : void
      {
      }
      
      protected function leaveQuenue(event:ButtonEvent) : void
      {
         ExternalInterface.call("leave_queue");
      }
   }
}

