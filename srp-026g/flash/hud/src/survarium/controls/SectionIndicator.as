package survarium.controls
{
   import flash.display.MovieClip;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.StatusIndicator;
   
   public class SectionIndicator extends StatusIndicator
   {
      
      public var track:Button;
      
      public var shiftInterval:uint = 1;
      
      protected var _itemRenderer:Class;
      
      protected var _cells:Array = new Array();
      
      protected var _scaleRenderers:Boolean = false;
      
      protected var _snapInterval:int = 1;
      
      protected var _useSections:Boolean = true;
      
      public function SectionIndicator()
      {
         super();
      }
      
      public function set renderer(value:String) : void
      {
         if(value == "")
         {
            return;
         }
         var classRef:Class = getDefinitionByName(value) as Class;
         if(classRef != null)
         {
            _itemRenderer = classRef;
         }
         else
         {
            trace("Error: " + this + ", The class " + value + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      override public function set value(new_value:Number) : void
      {
         if(new_value < minimum)
         {
            new_value = minimum;
         }
         else if(new_value > this.maximum)
         {
            new_value = this.maximum;
         }
         _value = new_value;
         this.draw();
      }
      
      public function set scaleRenders(value:Boolean) : void
      {
         _scaleRenderers = value;
      }
      
      override public function toString() : String
      {
         "[ SectionIndicator: min:" + minimum;
         return " max:" + this.maximum + " val:" + value + " snap:" + this.snapInterval + " ]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function updatePosition() : void
      {
         trace("SI::updatePosition",enabled);
         if(!enabled)
         {
            return;
         }
         setChildIndex(this.track,0);
         for(var i:uint = 0; i < this._cells.length; i++)
         {
            var cell:MovieClip;
            var pos:uint;
            var maxSize:int;
            cell = this._cells[i];
            if(this._useSections)
            {
               pos = i * this.snapInterval;
               if(pos < value)
               {
                  cell.gotoAndStop(1);
               }
               else
               {
                  cell.gotoAndStop(2);
               }
            }
            else
            {
               maxSize = this.track.width;
               cell.width = maxSize * (value / _maximum);
            }
         }
      }
      
      public function get snapInterval() : Number
      {
         return this._snapInterval;
      }
      
      public function set snapInterval(value:Number) : void
      {
         _snapInterval = value;
         invalidate(InvalidationType.SETTINGS);
      }
      
      override public function get maximum() : Number
      {
         return super.maximum;
      }
      
      override public function set maximum(value:Number) : void
      {
         if(value == _maximum)
         {
            return;
         }
         super.maximum = value;
         invalidate(InvalidationType.SETTINGS);
      }
      
      public function get useSections() : Boolean
      {
         return this._useSections;
      }
      
      public function set useSections(value:Boolean) : void
      {
         _useSections = value;
         invalidate(InvalidationType.SETTINGS);
      }
      
      override protected function draw() : void
      {
         var j:uint;
         var cellCount:uint;
         var maxSize:int;
         var rendererWidth:Number;
         var i:uint;
         var cell:MovieClip;
         if(isInvalid(InvalidationType.SETTINGS))
         {
            for(j = 0; j < this._cells.length; j++)
            {
               cell = this._cells[j];
               cell.gotoAndStop(1);
               removeChild(cell);
            }
            this._cells.length = 0;
            cellCount = this.useSections ? (this.maximum / this.snapInterval) : 1;
            maxSize = this.track.width;
            rendererWidth = (maxSize - (cellCount - 1) * this.shiftInterval) / cellCount;
            for(i = 0; i < cellCount; i++)
            {
               cell = new this._itemRenderer();
               cell.mouseEnabled = false;
               cell.mouseChildren = false;
               this._cells[i] = cell;
               cell.gotoAndStop(1);
               cell.width = rendererWidth;
               cell.x = this.track.x + i * (rendererWidth + this.shiftInterval);
               addChild(cell);
            }
         }
         this.updatePosition();
      }
   }
}

