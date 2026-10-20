package scaleform.clik.controls
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.interfaces.ITipRenderer;
   import scaleform.clik.interfaces.IToolTip;
   import scaleform.clik.managers.DragManager;
   
   public class ToolTip extends UIComponent implements IToolTip
   {
      
      protected static var _tip_interval:Timer = new Timer(300);
      
      protected static var _lastTarget:ToolTip;
      
      public var tipEnabled:Boolean = true;
      
      protected var _tip_content:Sprite;
      
      protected var _tip_contentName:String = "name";
      
      protected var _data:Object;
      
      protected var _tooltipRenderer:Class;
      
      protected var _tooltipRendererName:String;
      
      protected var _tipRenderer:ITipRenderer;
      
      protected var isOver:Boolean = false;
      
      public function ToolTip()
      {
         super();
         delay = 300;
      }
      
      public static function set delay(param1:int) : void
      {
         _tip_interval.delay = param1;
      }
      
      public static function get delay() : int
      {
         return _tip_interval.delay;
      }
      
      public function get tooltipRendererName() : String
      {
         return this._tooltipRendererName;
      }
      
      public function set tooltipRendererName(param1:String) : void
      {
         if(_inspector && param1 == "" || param1 == "")
         {
            return;
         }
         var _loc2_:Class = getDefinitionByName(param1) as Class;
         if(_loc2_ != null)
         {
            _tooltipRenderer = _loc2_;
         }
         else
         {
            trace("Error: " + this + ", The class " + param1 + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      public function set tipData(param1:Object) : void
      {
         if(this._data == param1)
         {
            return;
         }
         _data = param1;
      }
      
      public function get tipData() : Object
      {
         return this._data;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         addEventListener(MouseEvent.MOUSE_OVER,this.handleMouseMoveOverTip,false,0,true);
         addEventListener(MouseEvent.ROLL_OUT,this.handleMouseMoveOutTip,false,0,true);
         addEventListener(Event.REMOVED,this.handleDeleted,false,0,true);
         addEventListener("removedFromStage",this.handleDeleted,false,0,true);
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(this._data == null)
            {
               _tip_interval.reset();
            }
         }
      }
      
      protected function showTip(param1:Event = null) : *
      {
         if(!this.tipEnabled)
         {
            return;
         }
         if(!this._tooltipRenderer)
         {
            return;
         }
         if(!this._data)
         {
            return;
         }
         if(this._tooltipRenderer)
         {
            _tipRenderer = new this._tooltipRenderer();
         }
         if(!this._tipRenderer)
         {
            return;
         }
         if(!this._data || DragManager.inDrag())
         {
            _tip_interval.reset();
            return;
         }
         _tip_interval.reset();
         if(!this._tip_content)
         {
            if(this._data)
            {
               this._tipRenderer.data = this._data;
            }
            _tip_content = this._tipRenderer as Sprite;
            stage.addChild(this._tip_content);
            this._tip_content.mouseEnabled = false;
            this._tip_content.mouseChildren = false;
         }
         else
         {
            _tip_content = this._tipRenderer as Sprite;
            this._tipRenderer.data = this._data;
            stage.addChild(this._tip_content);
            this._tip_content.mouseEnabled = false;
            this._tip_content.mouseChildren = false;
         }
         this._tip_content.x = stage.mouseX;
         this._tip_content.y = stage.mouseY - this._tip_content.height;
         if(this._tip_content.y < 0)
         {
            this._tip_content.y = 4;
         }
         if(this._tip_content.x < 0)
         {
            this._tip_content.x = 4;
         }
         if(this._tip_content.y < 0)
         {
            this._tip_content.y = 4;
         }
         if(this._tip_content.x + this._tip_content.width > stage.stageWidth)
         {
            this._tip_content.x = stage.mouseX - this._tip_content.width + 4;
         }
         if(this._tip_content.y + this._tip_content.height > stage.stageHeight)
         {
            this._tip_content.y = stage.mouseY - this._tip_content.height - 4;
         }
      }
      
      protected function handleMouseMoveOverTip(param1:MouseEvent) : void
      {
         if(!this._tooltipRenderer)
         {
            return;
         }
         if(!DragManager.inDrag())
         {
            if((_lastTarget) && _lastTarget != this)
            {
               _lastTarget.handleDeleted(null);
            }
            if((this._tip_content) && (stage))
            {
               if(stage.contains(this._tip_content))
               {
                  stage.removeChild(this._tip_content);
               }
            }
            _lastTarget = this;
            _tip_interval.start();
            _tip_interval.addEventListener(TimerEvent.TIMER_COMPLETE,this.showTip);
            param1.stopPropagation();
         }
         isOver = true;
      }
      
      protected function handleMouseMoveOutTip(param1:MouseEvent) : void
      {
         if(!this._tooltipRenderer)
         {
            return;
         }
         isOver = false;
         if(_tip_interval)
         {
            _tip_interval.reset();
         }
         if(this._tip_content)
         {
            if((stage) && stage.contains(this._tip_content))
            {
               stage.removeChild(this._tip_content);
            }
         }
         if(_tip_interval)
         {
            _tip_interval.removeEventListener(TimerEvent.TIMER_COMPLETE,this.showTip);
         }
      }
      
      protected function handleMouseMoveTip(param1:MouseEvent) : void
      {
         if(!this.isOver)
         {
            return;
         }
         if(!this._tipRenderer)
         {
            return;
         }
         if(_tip_interval)
         {
            _tip_interval.reset();
            _tip_interval.start();
         }
         if(this._tip_content)
         {
            if(stage.contains(this._tip_content))
            {
               stage.removeChild(this._tip_content);
            }
         }
      }
      
      protected function handleDeleted(param1:Event) : void
      {
         if((this._tip_content) && (this._tip_content.parent) && this._tip_content.parent.contains(this._tip_content))
         {
            this._tip_content.parent.removeChild(this._tip_content);
         }
         if(_tip_interval)
         {
            _tip_interval.reset();
            _tip_interval.removeEventListener(TimerEvent.TIMER_COMPLETE,this.showTip);
         }
      }
   }
}

