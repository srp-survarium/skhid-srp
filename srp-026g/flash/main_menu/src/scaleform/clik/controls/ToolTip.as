package scaleform.clik.controls
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.geom.Rectangle;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.interfaces.ITipRenderer;
   import scaleform.clik.interfaces.IToolTip;
   import scaleform.clik.managers.DragManager;
   
   public class ToolTip extends UIComponent implements IToolTip
   {
      
      // Hover listens for TIMER_COMPLETE, so the delay must be one-shot (as the lobby movie's copy has it).
      protected static var _tip_interval:Timer = new Timer(300,1);
      
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
      
      public static function set delay(value:int) : void
      {
         _tip_interval.delay = value;
      }
      
      public static function get delay() : int
      {
         return _tip_interval.delay;
      }
      
      [Inspectable(name="tipRenderer",defaultValue="")]
      public function get tooltipRendererName() : String
      {
         return this._tooltipRendererName;
      }
      
      public function set tooltipRendererName(value:String) : void
      {
         var classRef:Class;
         if(_inspector && value == "" || value == "")
         {
            return;
         }
         try
         {
            classRef = getDefinitionByName(value) as Class;
         }
         catch(error:*)
         {
         }
         if(classRef != null)
         {
            _tooltipRenderer = classRef;
         }
         else
         {
            trace("Error: " + this + ", The class " + value + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      public function set tipData(value:Object) : void
      {
         if(this._data == value)
         {
            return;
         }
         _data = value;
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
         addEventListener(Event.REMOVED_FROM_STAGE,this.handleDeleted,false,0,true);
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(this._tooltipRenderer)
            {
               if(this._data == null)
               {
                  _tip_interval.reset();
               }
               if(this._tipRenderer && this._data)
               {
                  this._tipRenderer.data = this._data;
               }
            }
         }
      }
      
      protected function showTip(e:Event = null) : void
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
         // Adding the tip to the stage puts PopUpManager's canvas back on top
         // (its stage ADDED listener): a tip over a popup (the Vulkan options
         // panel) would be drawn under it. Raising the tip adds nothing.
         stage.setChildIndex(this._tip_content,stage.numChildren - 1);
         this.placeTip();
      }

      // srp: the tip under the control the pointer is on, never over it (above it when the window ends
      // first), from the pointer's x, inside the window; drawn at the scale the control is (the menu scales
      // itself: 2 at 4K), never below 1.
      protected function placeTip() : void
      {
         var scale:Number = Math.max(1,parent ? parent.transform.concatenatedMatrix.a : 1);
         this._tip_content.scaleX = scale;
         this._tip_content.scaleY = scale;
         var over:Rectangle = getBounds(stage);
         var gap:Number = 4 * scale;
         var tipWidth:Number = this._tip_content.width;
         var tipHeight:Number = this._tip_content.height;
         this._tip_content.x = Math.max(gap,Math.min(stage.mouseX + 16 * scale,stage.stageWidth - tipWidth - gap));
         this._tip_content.y = over.bottom + gap + tipHeight <= stage.stageHeight ? over.bottom + gap : Math.max(gap,over.top - gap - tipHeight);
      }
      
      protected function handleMouseMoveOverTip(event:MouseEvent) : void
      {
         if(!this._tooltipRenderer)
         {
            return;
         }
         if(!DragManager.inDrag())
         {
            if(_lastTarget && _lastTarget != this)
            {
               _lastTarget.handleDeleted(null);
            }
            if(this._tip_content && stage)
            {
               if(stage.contains(this._tip_content))
               {
                  stage.removeChild(this._tip_content);
               }
            }
            _lastTarget = this;
            _tip_interval.start();
            _tip_interval.addEventListener(TimerEvent.TIMER_COMPLETE,this.showTip);
            event.stopPropagation();
         }
         isOver = true;
      }
      
      protected function handleMouseMoveOutTip(event:MouseEvent) : void
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
            if(stage && stage.contains(this._tip_content))
            {
               stage.removeChild(this._tip_content);
            }
         }
         if(_tip_interval)
         {
            _tip_interval.removeEventListener(TimerEvent.TIMER_COMPLETE,this.showTip);
         }
      }
      
      protected function handleMouseMoveTip(event:MouseEvent) : void
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
      
      protected function handleDeleted(event:Event) : void
      {
         if(this._tip_content && this._tip_content.parent && this._tip_content.parent.contains(this._tip_content))
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

